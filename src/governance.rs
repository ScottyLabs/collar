use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct TeamFile {
    team: Group,
}

#[derive(Deserialize)]
struct Group {
    slug: String,
    name: String,
    #[serde(default)]
    leads: Vec<String>,
    #[serde(default)]
    members: Vec<String>,
    #[serde(default)]
    repos: Vec<Repository>,
    #[serde(default)]
    projects: Vec<Group>,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct Repository {
    pub name: String,
    #[serde(default)]
    pub features: BTreeMap<String, toml::Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub slug: String,
    pub name: String,
    pub team: String,
    pub leads: Vec<String>,
    pub members: Vec<String>,
    pub repos: Vec<Repository>,
    pub secret_namespace: Option<String>,
    pub source: PathBuf,
}

fn combine(parent: &[String], child: &[String]) -> Vec<String> {
    let mut result = BTreeMap::new();
    for value in parent.iter().chain(child) {
        result
            .entry(value.to_lowercase())
            .or_insert_with(|| value.clone());
    }
    result.into_values().collect()
}

pub fn projects(checkout: &Path) -> Result<Vec<Project>> {
    let directory = checkout.join("data/teams");
    let mut files = std::fs::read_dir(&directory)
        .with_context(|| format!("read governance teams at {}", directory.display()))?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    files.retain(|path| path.extension().is_some_and(|ext| ext == "toml"));
    files.sort();
    ensure!(
        !files.is_empty(),
        "governance checkout has no team TOML files"
    );
    let mut result = Vec::new();
    for file in files {
        let content =
            std::fs::read_to_string(&file).with_context(|| format!("read {}", file.display()))?;
        let team: TeamFile =
            toml::from_str(&content).with_context(|| format!("parse {}", file.display()))?;
        fn visit(
            group: &Group,
            team: &str,
            leads: &[String],
            members: &[String],
            source: &Path,
            result: &mut Vec<Project>,
        ) -> Result<()> {
            ensure!(
                !group.slug.trim().is_empty() && !group.name.trim().is_empty(),
                "governance group requires a nonempty slug and name"
            );
            let leads = combine(leads, &group.leads);
            let members = combine(members, &group.members);
            if !group.repos.is_empty() {
                ensure!(
                    group.repos.iter().all(|repo| !repo.name.trim().is_empty()),
                    "governance repository name must be nonempty"
                );
                result.push(Project {
                    slug: group.slug.clone(),
                    name: group.name.clone(),
                    team: team.into(),
                    leads: leads.clone(),
                    members: members.clone(),
                    repos: group.repos.clone(),
                    secret_namespace: group.projects.is_empty().then(|| group.slug.clone()),
                    source: source.to_owned(),
                });
            }
            for project in &group.projects {
                visit(project, team, &leads, &members, source, result)?;
            }
            Ok(())
        }
        visit(&team.team, &team.team.slug, &[], &[], &file, &mut result)?;
    }
    let mut repositories = BTreeSet::new();
    for project in &result {
        for repo in &project.repos {
            ensure!(
                repositories.insert(repo.name.to_lowercase()),
                "repository {} has ambiguous governance ownership",
                repo.name
            );
        }
    }
    result.sort_by(|a, b| a.slug.cmp(&b.slug));
    Ok(result)
}
