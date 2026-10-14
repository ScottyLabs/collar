use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::host_url;
use crate::{governance, repo::Repo};

#[derive(Deserialize)]
struct OrganizationFile {
    org: Organization,
}

#[derive(Deserialize)]
struct Organization {
    forgejo: Forge,
    keycloak: Identity,
}

#[derive(Deserialize)]
struct Forge {
    org: String,
    url: String,
}

#[derive(Deserialize)]
struct Identity {
    url: String,
    realm: String,
}

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
    pub features: BTreeMap<String, toml::Value>,
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
        println!(
            "{} ({})\nRepository: {}\nTeam: {}\nLeads: {}\nSecret namespace: {}",
            self.name,
            self.slug,
            self.repository,
            self.team,
            self.leads.join(", "),
            self.secret_namespace
                .as_deref()
                .unwrap_or("not defined by governance")
        );
        println!(
            "Features: {}\nGovernance: {}\nLink accounts: {}\nDeclared configuration; applied access and deployment health are not verified.",
            self.features.keys().cloned().collect::<Vec<_>>().join(", "),
            self.governance_source.display(),
            self.linked_accounts_url
        );
    }
}

pub fn resolve(checkout: &Path, repo: &Repo) -> Result<Project> {
    let organization_source = checkout.join("data/org.toml");
    let text = std::fs::read_to_string(&organization_source)
        .context("read governance organization at data/org.toml")?;
    let organization: OrganizationFile = crate::document::parse(&text, "governance organization")
        .context(
        "parse governance organization; expected org.forgejo org/url and org.keycloak url/realm",
    )?;
    let org = organization.org;
    ensure!(
        repo.origin == host_url(&org.forgejo.url)?,
        "selected repository is on a different forge from governance"
    );
    ensure!(
        repo.owner.eq_ignore_ascii_case(&org.forgejo.org),
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
        host_url(&org.keycloak.url).context("invalid governance identity host")?;
    ensure!(
        !org.keycloak.realm.trim().is_empty(),
        "governance identity realm must be nonempty"
    );
    linked_accounts_url
        .path_segments_mut()
        .map_err(|()| anyhow::anyhow!("identity origin cannot contain paths"))?
        .extend([
            "realms",
            &org.keycloak.realm,
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
    let text = std::fs::read_to_string(checkout.join("data/org.toml"))
        .context("read governance organization")?;
    let organization: OrganizationFile = crate::document::parse(&text, "governance organization")?;
    ensure!(
        !organization.org.forgejo.org.trim().is_empty(),
        "governance organization must be nonempty"
    );
    host_url(&organization.org.forgejo.url)
}
