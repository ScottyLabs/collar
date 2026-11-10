use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Serialize;
use url::Url;

use crate::client::host_url;
use crate::{governance, repo::Repo};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub repository: String,
    pub url: Url,
    pub slug: String,
    pub name: String,
    pub team: String,
    pub leads: Vec<String>,
    pub members: Vec<String>,
    pub features: BTreeMap<String, serde_json::Value>,
    pub secret_namespace: Option<String>,
    pub governance_source: PathBuf,
    pub organization_source: PathBuf,
    pub linked_accounts_url: Url,
    pub declared_only: bool,
}

impl Project {
    pub const FIELDS: &[&str] = &[
        "repository",
        "url",
        "slug",
        "name",
        "team",
        "leads",
        "members",
        "features",
        "secretNamespace",
        "governanceSource",
        "organizationSource",
        "linkedAccountsUrl",
        "declaredOnly",
    ];

    pub fn print(&self) {
        crate::ui::heading(&format!("{} ({})", self.name, self.slug));
        crate::ui::detail("Repository", &self.repository);
        crate::ui::detail("Team", &self.team);
        crate::ui::detail("Leads", &self.leads.join(", "));
        crate::ui::detail(
            "Secret namespace",
            self.secret_namespace
                .as_deref()
                .unwrap_or("not defined by governance"),
        );
        crate::ui::table(
            &["Feature", "Configuration"],
            &self
                .features
                .iter()
                .map(|(name, value)| vec![name.clone(), value.to_string()])
                .collect::<Vec<_>>(),
        );
        crate::ui::detail("Governance", &self.governance_source.display().to_string());
        crate::ui::detail("Link accounts", self.linked_accounts_url.as_str());
        crate::ui::detail(
            "Verification",
            "Declared configuration; applied access and deployment health are not verified.",
        );
    }
}

pub fn resolve(checkout: &Path, repo: &Repo) -> Result<Project> {
    let organization_source = governance::data_directory(checkout);
    let data = governance::load(checkout)?;
    let (forgejo, keycloak) = (governance::forgejo(&data)?, governance::keycloak(&data)?);
    ensure!(
        repo.origin == host_url(&forgejo.url)?,
        "selected repository is on a different forge from governance"
    );
    ensure!(
        repo.owner.eq_ignore_ascii_case(&forgejo.org),
        "selected repository is outside the governance organization; select its canonical upstream with -R"
    );

    let mut projects = governance::projects(checkout)?;
    let index = projects.iter().position(|project| project.repos.iter().any(|entry| entry.name.eq_ignore_ascii_case(&repo.name)))
        .with_context(|| format!("repository {} is not declared in governance; check the checkout revision and canonical upstream", repo.name))?;
    let project = projects.remove(index);
    let features = project
        .repos
        .into_iter()
        .find(|entry| entry.name.eq_ignore_ascii_case(&repo.name))
        .context("matched governance project has no repository")?
        .features;
    let mut url = repo.origin.clone();
    url.path_segments_mut()
        .map_err(|()| anyhow::anyhow!("repository origin cannot contain paths"))?
        .extend([&repo.owner, &repo.name]);
    let mut linked_accounts_url =
        host_url(&keycloak.url).context("invalid governance identity host")?;
    ensure!(
        !keycloak.realm.trim().is_empty(),
        "governance identity realm must be nonempty"
    );
    linked_accounts_url
        .path_segments_mut()
        .map_err(|()| anyhow::anyhow!("identity origin cannot contain paths"))?
        .extend([
            "realms",
            &keycloak.realm,
            "account",
            "account-security",
            "linked-accounts",
        ]);
    Ok(Project {
        repository: format!("{}/{}", repo.owner, repo.name),
        url,
        slug: project.slug,
        name: project.name,
        team: project.team,
        leads: project.leads,
        members: project.members,
        features,
        secret_namespace: project.secret_namespace,
        governance_source: project.source,
        organization_source,
        linked_accounts_url,
        declared_only: true,
    })
}

pub fn organization_origin(checkout: &Path) -> Result<Url> {
    let data = governance::load(checkout)?;
    let forgejo = governance::forgejo(&data)?;
    ensure!(
        !forgejo.org.trim().is_empty(),
        "governance organization must be nonempty"
    );

    host_url(&forgejo.url)
}

pub fn repository(checkout: &Path, name: &str) -> Result<Repo> {
    let data = governance::load(checkout)?;
    let forgejo = governance::forgejo(&data)?;
    Repo::parse(&format!("{}/{name}", forgejo.org), Some(&forgejo.url))
}
