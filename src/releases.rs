use std::{
    fs,
    io::{self, Read},
    path::PathBuf,
};

use anyhow::{Context, Result, bail, ensure};
use clap::Subcommand;
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{client::Api, output::Output, repo::Repo, ui};

#[derive(Subcommand, Debug)]
pub enum Command {
    /// List releases, newest first.
    List {
        #[arg(long, default_value_t = 30)]
        limit: usize,
        #[command(flatten)]
        output: Output,
    },
    /// View one release by tag.
    View {
        tag: String,
        #[command(flatten)]
        output: Output,
    },
    /// Create a release for an existing tag.
    Create {
        tag: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, conflicts_with = "notes_file")]
        notes: Option<String>,
        #[arg(long)]
        notes_file: Option<PathBuf>,
        #[arg(long)]
        draft: bool,
        #[arg(long)]
        prerelease: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Edit a release by tag.
    Edit {
        tag: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, conflicts_with = "notes_file")]
        notes: Option<String>,
        #[arg(long)]
        notes_file: Option<PathBuf>,
        #[arg(long, conflicts_with = "publish")]
        draft: bool,
        #[arg(long)]
        publish: bool,
        #[arg(long, conflicts_with = "stable")]
        prerelease: bool,
        #[arg(long)]
        stable: bool,
        #[command(flatten)]
        output: Output,
    },
    /// Delete a release by tag.
    Delete { tag: String },
}

#[derive(Debug, Deserialize, Serialize)]
struct Release {
    id: i64,
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    draft: bool,
    prerelease: bool,
    html_url: Option<String>,
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize, Serialize)]
struct Asset {
    name: Option<String>,
    browser_download_url: Option<String>,
    size: Option<u64>,
}

const FIELDS: &[&str] = &[
    "id",
    "tag_name",
    "name",
    "body",
    "draft",
    "prerelease",
    "html_url",
    "assets",
];

pub fn run(command: Command, api: &Api, repo: &Repo) -> Result<()> {
    let mut path = repo.path().to_vec();
    path.push("releases");
    match command {
        Command::List { limit, output } => {
            output.validate_fields(FIELDS)?;
            let releases: Vec<Release> = api.list(&path, &[], limit)?;
            output.print(&releases, || {
                ui::heading("Releases");
                ui::table(
                    &["Tag", "Name", "Status", "Assets"],
                    &releases
                        .iter()
                        .map(|release| {
                            vec![
                                release.tag_name.clone(),
                                release.name.clone().unwrap_or_default(),
                                if release.draft {
                                    "draft".into()
                                } else if release.prerelease {
                                    "prerelease".into()
                                } else {
                                    "published".into()
                                },
                                release.assets.len().to_string(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
        Command::View { tag, output } => {
            valid_tag(&tag)?;
            output.validate_fields(FIELDS)?;
            let release = by_tag(api, &path, &tag)?;
            output.print(&release, || print_release(&release))
        }
        Command::Create {
            tag,
            title,
            notes,
            notes_file,
            draft,
            prerelease,
            output,
        } => {
            valid_tag(&tag)?;
            if let Some(title) = &title {
                valid_title(title)?;
            }
            output.validate_fields(FIELDS)?;
            api.require_token()?;
            let body = notes_body(notes, notes_file)?;
            let mut payload =
                json!({"tag_name": tag, "body": body, "draft": draft, "prerelease": prerelease});
            if let Some(title) = title {
                payload["name"] = Value::String(title);
            }
            let release: Release = api.json(Method::POST, &path, &[], Some(&payload))?;
            output.print(&release, || print_release(&release))
        }
        Command::Edit {
            tag,
            title,
            notes,
            notes_file,
            draft,
            publish,
            prerelease,
            stable,
            output,
        } => {
            valid_tag(&tag)?;
            output.validate_fields(FIELDS)?;
            ensure!(
                title.is_some()
                    || notes.is_some()
                    || notes_file.is_some()
                    || draft
                    || publish
                    || prerelease
                    || stable,
                "specify at least one field to edit"
            );
            if let Some(title) = &title {
                valid_title(title)?;
            }
            api.require_token()?;
            let mut payload = serde_json::Map::new();
            if let Some(title) = title {
                payload.insert("name".into(), Value::String(title));
            }
            if notes.is_some() || notes_file.is_some() {
                payload.insert("body".into(), Value::String(notes_body(notes, notes_file)?));
            }
            if draft || publish {
                payload.insert("draft".into(), Value::Bool(draft));
            }
            if prerelease || stable {
                payload.insert("prerelease".into(), Value::Bool(prerelease));
            }
            let release = by_tag(api, &path, &tag)?;
            let id = release.id.to_string();
            path.push(&id);
            let updated: Release =
                api.json(Method::PATCH, &path, &[], Some(&Value::Object(payload)))?;
            output.print(&updated, || print_release(&updated))
        }
        Command::Delete { tag } => {
            valid_tag(&tag)?;
            api.require_token()?;
            let release = by_tag(api, &path, &tag)?;
            let id = release.id.to_string();
            path.push(&id);
            api.text(Method::DELETE, &path, &[], None)?;
            println!("Deleted release {tag}");
            Ok(())
        }
    }
}

fn by_tag(api: &Api, base: &[&str], tag: &str) -> Result<Release> {
    let mut path = base.to_vec();
    path.extend(["tags", tag]);
    api.json(Method::GET, &path, &[], None).map_err(|error| {
        if error.to_string().contains("HTTP 404") {
            anyhow::anyhow!("release tag {tag} was not found")
        } else {
            error
        }
    })
}

fn valid_tag(tag: &str) -> Result<()> {
    ensure!(
        !tag.trim().is_empty() && !tag.contains(['\r', '\n', '\0']),
        "tag must be nonempty and contain no control line breaks"
    );
    Ok(())
}

fn valid_title(title: &str) -> Result<()> {
    ensure!(
        !title.trim().is_empty() && !title.contains(['\r', '\n', '\0']),
        "title must be nonempty and contain no control line breaks"
    );
    Ok(())
}

fn notes_body(notes: Option<String>, path: Option<PathBuf>) -> Result<String> {
    match (notes, path) {
        (Some(body), None) => Ok(body),
        (None, Some(path)) if path.as_os_str() == "-" => {
            let mut body = String::new();
            io::stdin()
                .read_to_string(&mut body)
                .context("read release notes from stdin")?;
            Ok(body)
        }
        (None, Some(path)) => fs::read_to_string(&path)
            .with_context(|| format!("read release notes from {}", path.display())),
        (None, None) => Ok(String::new()),
        (Some(_), Some(_)) => bail!("choose either --notes or --notes-file"),
    }
}

fn print_release(release: &Release) {
    ui::heading(&format!(
        "{} - {}",
        release.tag_name,
        release.name.as_deref().unwrap_or("")
    ));
    ui::detail(
        "Status",
        if release.draft {
            "draft"
        } else if release.prerelease {
            "prerelease"
        } else {
            "published"
        },
    );
    if let Some(url) = &release.html_url {
        ui::detail("URL", url);
    }
    if !release.assets.is_empty() {
        ui::table(
            &["Asset", "Download", "Size"],
            &release
                .assets
                .iter()
                .map(|asset| {
                    vec![
                        asset.name.clone().unwrap_or_default(),
                        asset.browser_download_url.clone().unwrap_or_default(),
                        asset
                            .size
                            .map(|size| format!("{size} bytes"))
                            .unwrap_or_default(),
                    ]
                })
                .collect::<Vec<_>>(),
        );
    }
    if let Some(body) = release
        .body
        .as_deref()
        .filter(|body| !body.trim().is_empty())
    {
        println!("\n{body}");
    }
}
