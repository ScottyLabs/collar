use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use governance_schema::team::{self, GroupFields, Repo, Team, TeamFile};
use governance_schema::tech::{ForgejoConfig, KeycloakConnection, TechConfig};
use serde::{Deserialize, Serialize};

/// Governance's normalized data, generated from `data/` by `governance generate`
pub const DATA: &str = "generated/data.json";

#[derive(Deserialize)]
pub struct Data {
    tech: TechConfig,
    teams: Vec<TeamFile>,
}

impl Data {
    pub fn forgejo(&self) -> Result<&ForgejoConfig> {
        self.tech
            .forgejo
            .as_ref()
            .context("governance data declares no Forgejo organization")
    }

    pub fn keycloak(&self) -> Result<&KeycloakConnection> {
        self.tech
            .keycloak
            .as_ref()
            .context("governance data declares no Keycloak realm")
    }
}

pub fn load(checkout: &Path) -> Result<Data> {
    let text = std::fs::read_to_string(checkout.join(DATA))
        .with_context(|| format!("read governance data at {DATA}"))?;
    serde_json::from_str(&text).map_err(|error| {
        anyhow::anyhow!(
            "invalid governance data at line {}, column {}; update Collar if governance added fields",
            error.line(),
            error.column()
        )
    })
}

#[derive(Serialize, Clone)]
pub struct Repository {
    pub name: String,
    pub features: BTreeMap<String, serde_json::Value>,
}

impl Repository {
    /// Keeps only the enabled features, which governance serializes as non-null
    fn new(repo: &Repo) -> Result<Self> {
        let features = match serde_json::to_value(&repo.features)? {
            serde_json::Value::Object(features) => features
                .into_iter()
                .filter(|(_, config)| !config.is_null())
                .collect(),
            _ => BTreeMap::new(),
        };
        Ok(Self {
            name: repo.name.clone(),
            features,
        })
    }
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

/// The teams and projects that own repositories
pub fn projects(checkout: &Path) -> Result<Vec<Project>> {
    let data = load(checkout)?;
    ensure!(!data.teams.is_empty(), "governance data declares no teams");

    let source = checkout.join(DATA);
    let mut result = Vec::new();
    for TeamFile { team } in &data.teams {
        let groups = std::iter::once(&team.group).chain(team.projects.iter().map(|p| &p.group));
        for group in groups.filter(|group| !group.repos.is_empty()) {
            result.push(project(&data.teams, team, group, &source)?);
        }
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

/// A group's view of its team's leads, with members from both the team and the group
fn project(teams: &[TeamFile], team: &Team, group: &GroupFields, source: &Path) -> Result<Project> {
    let repos = group
        .repos
        .iter()
        .map(Repository::new)
        .collect::<Result<Vec<_>>>()?;
    let members: BTreeSet<&String> = team.group.members.iter().chain(&group.members).collect();
    let secret_namespace = group
        .repos
        .first()
        .and_then(|repo| team::secret_namespace_of(teams, &repo.name))
        .map(str::to_owned);

    Ok(Project {
        slug: group.slug.clone(),
        name: group.name.clone(),
        team: team.group.slug.clone(),
        leads: team.leads.iter().cloned().collect(),
        members: members.into_iter().cloned().collect(),
        repos,
        secret_namespace,
        source: source.to_owned(),
    })
}
