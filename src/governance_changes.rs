use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};
use clap::{Args, Subcommand};
use reqwest::Method;
use serde_json::Value;
use toml_edit::{DocumentMut, Item, Table, Value as TomlValue};

use crate::{
    client::Api,
    project,
    repo::Repo,
    source::{Kind, Sources},
};

#[derive(Subcommand)]
pub enum Command {
    /// Add your Forgejo username to a team's or project's membership.
    Join(MembershipArgs),
    /// Remove your Forgejo username from a team's or project's membership.
    Leave(MembershipArgs),
    /// Enable a declared governance feature for the selected repository.
    Enable(FeatureArgs),
    /// Disable a declared governance feature for the selected repository.
    Disable(FeatureArgs),
}

#[derive(Args)]
pub struct MembershipArgs {
    /// Writable governance working checkout.
    #[arg(long, required = true)]
    checkout: PathBuf,
    /// Exact team slug from data/teams/<slug>.toml.
    #[arg(long, conflicts_with = "project")]
    team: Option<String>,
    /// Exact nested project path, such as terrier/website.
    #[arg(long)]
    project: Option<String>,
}

#[derive(Args)]
pub struct FeatureArgs {
    /// Writable governance working checkout.
    #[arg(long, required = true)]
    checkout: PathBuf,
    /// Governance feature identifier.
    #[arg(long, value_parser = ["ai_gateway", "cdn", "docs", "kennel", "mobile_app", "oidc_client", "posthog", "sentry"])]
    feature: String,
    /// TOML or JSON table containing feature settings (defaults to an empty table).
    #[arg(long)]
    config: Option<PathBuf>,
}

pub fn run(command: Command, api: &Api, repo: &Repo, sources: &Sources<'_>) -> Result<()> {
    match command {
        Command::Join(args) => membership(args, api, repo, sources, true),
        Command::Leave(args) => membership(args, api, repo, sources, false),
        Command::Enable(args) => feature(args, api, repo, sources, true),
        Command::Disable(args) => feature(args, api, repo, sources, false),
    }
}

fn membership(
    args: MembershipArgs,
    api: &Api,
    repo: &Repo,
    sources: &Sources<'_>,
    join: bool,
) -> Result<()> {
    let checkout = working_checkout(&args.checkout)?;
    let source = sources.load_at(
        Kind::Governance,
        Some(checkout.clone()),
        false,
        &repo.origin,
    )?;
    let declared_origin = project::organization_origin(&source.directory)?;
    ensure!(
        declared_origin == repo.origin,
        "selected repository forge does not match governance organization"
    );
    api.require_token()?;
    let username: Value = api
        .json(Method::GET, &["user"], &[], None)
        .context("identify authenticated Forgejo user")?;
    let username = username
        .get("login")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .context("Forgejo /user response did not include a login")?;
    let (file, path) = membership_target(
        &source.directory,
        args.team.as_deref(),
        args.project.as_deref(),
    )?;
    let mut added = false;
    let mut removed = false;
    update(&file, &checkout, |doc| {
        let inherited = inherited_member(doc, &path, username)?;
        let group = group_at_mut(doc, &path)?;
        let lead = group
            .get("leads")
            .and_then(Item::as_array)
            .is_some_and(|values| {
                values
                    .iter()
                    .filter_map(TomlValue::as_str)
                    .any(|value| value.eq_ignore_ascii_case(username))
            });
        if join && lead {
            return Ok(());
        }
        if !join && lead {
            bail!(
                "{username} is a lead in {}; leads cannot remove their own lead role with membership leave",
                path.join("/")
            );
        }
        let members = group
            .entry("members")
            .or_insert(Item::Value(TomlValue::Array(toml_edit::Array::new())));
        let values = members
            .as_array_mut()
            .context("membership members must be a TOML array")?;
        let exists = values
            .iter()
            .filter_map(TomlValue::as_str)
            .any(|value| value.eq_ignore_ascii_case(username));
        if !join && inherited && !exists {
            bail!(
                "{username} inherits membership through {}; choose that ancestor selector to remove it",
                path[..path.len().saturating_sub(1)].join("/")
            );
        }
        if join && inherited {
            return Ok(());
        }
        if join && !exists {
            values.push(username);
            added = true;
        }
        if !join {
            removed = exists;
            let mut kept = toml_edit::Array::new();
            for value in values.iter() {
                if value
                    .as_str()
                    .is_none_or(|value| !value.eq_ignore_ascii_case(username))
                {
                    kept.push(value.clone());
                }
            }
            *values = kept;
        }
        Ok(())
    })?;
    if join {
        if added {
            println!(
                "Added {} to {}. Review and submit the governance change as a pull request.",
                username,
                path.join("/")
            );
        } else {
            println!(
                "{} is already covered by membership in {}; no TOML change was needed.",
                username,
                path.join("/")
            );
        }
    } else if removed {
        println!(
            "Removed {} from {}. Review and submit the governance change as a pull request.",
            username,
            path.join("/")
        );
    } else {
        println!(
            "{} is not a direct member of {}; no TOML change was needed.",
            username,
            path.join("/")
        );
    }
    Ok(())
}

fn inherited_member(doc: &DocumentMut, path: &[String], username: &str) -> Result<bool> {
    let mut group = doc
        .get("team")
        .and_then(Item::as_table)
        .context("team declaration is missing [team]")?;
    for index in 0..path.len().saturating_sub(1) {
        ensure!(
            group.get("slug").and_then(Item::as_str) == Some(path[index].as_str()),
            "membership selector no longer matches team declaration"
        );
        let is_listed = |field: &str| {
            group
                .get(field)
                .and_then(Item::as_array)
                .is_some_and(|values| {
                    values
                        .iter()
                        .filter_map(TomlValue::as_str)
                        .any(|value| value.eq_ignore_ascii_case(username))
                })
        };
        if is_listed("members") || is_listed("leads") {
            return Ok(true);
        }
        let next = &path[index + 1];
        group = group
            .get("projects")
            .and_then(Item::as_array_of_tables)
            .and_then(|projects| {
                projects
                    .iter()
                    .find(|item| item.get("slug").and_then(Item::as_str) == Some(next))
            })
            .with_context(|| format!("missing nested project {next}"))?;
    }
    Ok(false)
}

fn feature(
    args: FeatureArgs,
    api: &Api,
    repo: &Repo,
    sources: &Sources<'_>,
    enable: bool,
) -> Result<()> {
    ensure!(
        enable || args.config.is_none(),
        "--config can only be used when enabling a feature"
    );
    let checkout = working_checkout(&args.checkout)?;
    let source = sources.load_at(
        Kind::Governance,
        Some(checkout.clone()),
        false,
        &repo.origin,
    )?;
    let resolved = project::resolve(&source.directory, repo)?;
    let file = resolved.governance_source;
    let repo_name = repo.name.clone();
    let setting = args
        .config
        .map(read_feature_config)
        .transpose()?
        .unwrap_or_else(|| Item::Table(Table::new()));
    update(&file, &checkout, |doc| {
        let repo_item = find_repo(doc, &repo_name)?;
        let features = repo_item
            .entry("features")
            .or_insert(Item::Table(Table::new()))
            .as_table_mut()
            .context("repository features must be a TOML table")?;
        if enable {
            features.insert(&args.feature, setting.clone());
        } else {
            features.remove(&args.feature);
        }
        Ok(())
    })?;
    println!(
        "{} feature {} for {}/{}. Review and submit the governance change as a pull request.",
        if enable { "Enabled" } else { "Disabled" },
        args.feature,
        repo.owner,
        repo.name
    );
    let _ = api;
    Ok(())
}

fn working_checkout(path: &Path) -> Result<PathBuf> {
    let path = path
        .canonicalize()
        .with_context(|| format!("resolve governance checkout {}", path.display()))?;
    ensure!(
        path.join("Cargo.toml").is_file()
            && path.join("data/org.toml").is_file()
            && path.join("data/teams").is_dir(),
        "{} is not a governance working checkout",
        path.display()
    );
    Ok(path)
}

fn membership_target(
    checkout: &Path,
    team: Option<&str>,
    project: Option<&str>,
) -> Result<(PathBuf, Vec<String>)> {
    ensure!(
        team.is_some() ^ project.is_some(),
        "specify exactly one of --team or --project"
    );
    let (team_slug, path) = if let Some(team) = team {
        (team.to_owned(), vec![team.to_owned()])
    } else {
        let parts = project
            .context("missing project selector")?
            .split('/')
            .map(str::to_owned)
            .collect::<Vec<_>>();
        ensure!(
            parts.len() >= 2 && parts.iter().all(|part| !part.is_empty()),
            "project selector must be team/project[/nested-project]"
        );
        (parts[0].clone(), parts)
    };
    let mut selected = None;
    for entry in std::fs::read_dir(checkout.join("data/teams"))? {
        let file = entry?.path();
        if file.extension().is_none_or(|extension| extension != "toml") {
            continue;
        }
        let doc = std::fs::read_to_string(&file)?.parse::<DocumentMut>()?;
        if doc
            .get("team")
            .and_then(|team| team.get("slug"))
            .and_then(Item::as_str)
            == Some(&team_slug)
        {
            ensure!(
                selected.is_none(),
                "ambiguous team declaration for slug {team_slug}"
            );
            selected = Some((file, doc));
        }
    }
    let (file, doc) =
        selected.with_context(|| format!("no team declaration for exact slug {team_slug}"))?;
    let _ = group_at(&doc, &path)?;
    Ok((file, path))
}

fn group_at<'a>(doc: &'a DocumentMut, path: &[String]) -> Result<&'a Table> {
    let mut group = doc
        .get("team")
        .and_then(Item::as_table)
        .context("team declaration is missing [team]")?;
    ensure!(
        path.first()
            .is_some_and(|slug| group.get("slug").and_then(Item::as_str) == Some(slug)),
        "team selector does not exactly match declared slug"
    );
    for slug in &path[1..] {
        group = group
            .get("projects")
            .and_then(Item::as_array_of_tables)
            .and_then(|projects| {
                projects
                    .iter()
                    .find(|p| p.get("slug").and_then(Item::as_str) == Some(slug))
            })
            .with_context(|| {
                format!("project selector {slug} does not exactly match a nested project slug")
            })?;
    }
    Ok(group)
}

fn group_at_mut<'a>(doc: &'a mut DocumentMut, path: &[String]) -> Result<&'a mut Table> {
    let mut group = doc
        .get_mut("team")
        .and_then(Item::as_table_mut)
        .context("team declaration is missing [team]")?;
    ensure!(
        path.first()
            .is_some_and(|slug| group.get("slug").and_then(Item::as_str) == Some(slug)),
        "team selector does not exactly match declared slug"
    );
    for slug in &path[1..] {
        group = group
            .get_mut("projects")
            .and_then(Item::as_array_of_tables_mut)
            .and_then(|projects| {
                projects
                    .iter_mut()
                    .find(|p| p.get("slug").and_then(Item::as_str) == Some(slug))
            })
            .with_context(|| {
                format!("project selector {slug} does not exactly match a nested project slug")
            })?;
    }
    Ok(group)
}

fn find_repo<'a>(doc: &'a mut DocumentMut, name: &str) -> Result<&'a mut Table> {
    fn walk<'a>(group: &'a mut Table, name: &str) -> Option<&'a mut Table> {
        let index = group
            .get("repos")
            .and_then(Item::as_array_of_tables)
            .and_then(|repos| {
                repos.iter().position(|r| {
                    r.get("name")
                        .and_then(Item::as_str)
                        .is_some_and(|n| n.eq_ignore_ascii_case(name))
                })
            });
        if let Some(index) = index {
            return group
                .get_mut("repos")?
                .as_array_of_tables_mut()?
                .get_mut(index);
        }
        if let Some(projects) = group
            .get_mut("projects")
            .and_then(Item::as_array_of_tables_mut)
        {
            for project in projects.iter_mut() {
                if let Some(found) = walk(project, name) {
                    return Some(found);
                }
            }
        }
        None
    }
    walk(
        doc.get_mut("team")
            .and_then(Item::as_table_mut)
            .context("team declaration is missing [team]")?,
        name,
    )
    .with_context(|| format!("repository {name} was not found in its canonical governance source"))
}

fn read_feature_config(path: PathBuf) -> Result<Item> {
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("read feature config {}", path.display()))?;
    let item: Item = if path.extension().is_some_and(|ext| ext == "json") {
        let value: Value = serde_json::from_str(&text).context("parse feature config JSON")?;
        let toml_value =
            toml::Value::try_from(value).context("convert feature config JSON to TOML")?;
        ensure!(
            toml_value.is_table(),
            "feature config must be a TOML object/table"
        );
        let toml_text = toml::to_string(&toml_value).context("serialize feature config")?;
        toml_text
            .parse::<DocumentMut>()
            .context("feature config must be a TOML table")?
            .as_table()
            .clone()
            .into()
    } else {
        text.parse::<DocumentMut>()
            .context("parse feature config TOML")?
            .as_table()
            .clone()
            .into()
    };
    ensure!(
        item.is_table(),
        "feature config must contain a table/object"
    );
    Ok(item)
}

fn update(
    path: &Path,
    checkout: &Path,
    edit: impl FnOnce(&mut DocumentMut) -> Result<()>,
) -> Result<()> {
    let permissions = std::fs::metadata(path)
        .with_context(|| format!("inspect {}", path.display()))?
        .permissions();
    let original =
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut doc = original
        .parse::<DocumentMut>()
        .with_context(|| format!("parse {}", path.display()))?;
    edit(&mut doc)?;
    if doc.to_string() == original {
        return Ok(());
    }
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().context("governance source has no parent")?)
            .context("create temporary governance edit")?;
    use std::io::Write;
    temp.write_all(doc.to_string().as_bytes())
        .context("write governance edit")?;
    temp.as_file()
        .set_permissions(permissions.clone())
        .context("preserve governance file permissions")?;
    temp.as_file().sync_all().context("sync governance edit")?;
    temp.persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("replace {}", path.display()))?;
    if let Err(validation_error) = validate(checkout) {
        if let Err(rollback_error) = restore_original(path, &original, permissions) {
            return Err(rollback_error.context(format!(
                "governance validation also failed: {validation_error:#}"
            )));
        }
        return Err(validation_error)
            .context("validate governance change; original file was restored");
    }
    Ok(())
}

fn restore_original(path: &Path, original: &str, permissions: std::fs::Permissions) -> Result<()> {
    let mut restore =
        tempfile::NamedTempFile::new_in(path.parent().context("governance source has no parent")?)
            .context("create rollback file")?;
    use std::io::Write;
    restore
        .write_all(original.as_bytes())
        .context("write rollback file")?;
    restore
        .as_file()
        .set_permissions(permissions)
        .context("preserve governance file permissions during rollback")?;
    restore.as_file().sync_all().context("sync rollback file")?;
    restore
        .persist(path)
        .map_err(|e| e.error)
        .with_context(|| format!("restore {} after validation failure", path.display()))?;
    Ok(())
}

fn validate(checkout: &Path) -> Result<()> {
    let mut command = std::process::Command::new("cargo");
    command
        .args(["run", "-p", "governance", "--", "validate"])
        .current_dir(checkout);
    crate::process::run(&mut command).context("run canonical governance validator")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn membership_targets_resolve_nested_projects_and_exact_case() {
        let root = tempfile::tempdir().unwrap();
        let teams = root.path().join("data/teams");
        std::fs::create_dir_all(&teams).unwrap();
        let file = teams.join("tech.toml");
        std::fs::write(
            &file,
            "[team]\nslug='tech'\nname='Tech'\n[[team.projects]]\nslug='web'\nname='Web'\n[[team.projects.projects]]\nslug='docs'\nname='Docs'\n",
        )
        .unwrap();
        let (selected, path) = membership_target(root.path(), None, Some("tech/web/docs")).unwrap();
        assert_eq!(selected, file);
        assert_eq!(path, ["tech", "web", "docs"]);
        assert!(membership_target(root.path(), None, Some("tech/Web/docs")).is_err());
    }

    #[test]
    fn inherited_membership_is_detected_without_editing_ancestor() {
        let doc = "[team]\nslug='tech'\nname='Tech'\nmembers=['alice']\n[[team.projects]]\nslug='web'\nname='Web'\n"
            .parse::<DocumentMut>()
            .unwrap();
        assert!(inherited_member(&doc, &["tech".into(), "web".into()], "alice").unwrap());
        assert!(!inherited_member(&doc, &["tech".into(), "web".into()], "bob").unwrap());
        assert!(!inherited_member(&doc, &["tech".into()], "alice").unwrap());
    }
}
