use super::{Pull, Status, User};
use crate::{client::Api, repo::Repo};
use anyhow::{Context, Result, ensure};
use reqwest::Method;
use std::collections::BTreeMap;
use url::Url;

pub fn list(api: &Api, repo: &Repo, state: &str, limit: usize) -> Result<Vec<Pull>> {
    filtered_list(api, repo, state, limit, &Filters::default())
}

#[derive(Default)]
pub struct Filters {
    pub author: Option<String>,
    pub base: Option<String>,
    pub head: Option<String>,
}

pub fn filtered_list(
    api: &Api,
    repo: &Repo,
    state: &str,
    limit: usize,
    filters: &Filters,
) -> Result<Vec<Pull>> {
    ensure!(limit > 0, "list limit must be positive");
    let mut query = vec![("state".into(), state.into())];
    for (key, value) in [
        ("poster", &filters.author),
        ("base", &filters.base),
        ("head", &filters.head),
    ] {
        if let Some(value) = value {
            ensure!(!value.trim().is_empty(), "{key} filter must be nonempty");
            let value = if key == "poster" && value == "@me" {
                api.require_token()?;
                api.json::<User>(Method::GET, &["user"], &[], None)?.login
            } else {
                value.clone()
            };
            query.push((key.into(), value));
        }
    }
    let mut path = repo.path().to_vec();
    path.push("pulls");
    api.list(&path, &query, limit)
}

pub fn select(api: &Api, repo: &Repo, selector: Option<&str>) -> Result<u64> {
    if let Some(selector) = selector {
        if !selector.is_empty() && selector.bytes().all(|byte| byte.is_ascii_digit()) {
            let number: u64 = selector.parse().context("PR number is too large")?;
            ensure!(number > 0, "PR number must be positive");
            return Ok(number);
        }
        if selector.contains("://") {
            let url = Url::parse(selector).context("invalid PR URL")?;
            ensure!(
                url.origin() == repo.origin.origin()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.query().is_none()
                    && url.fragment().is_none(),
                "PR URL must belong to the selected forge and contain no credentials or query"
            );
            let parts: Vec<_> = url.path().trim_matches('/').split('/').collect();
            ensure!(
                parts.len() == 4
                    && parts[0].eq_ignore_ascii_case(&repo.owner)
                    && parts[1].eq_ignore_ascii_case(&repo.name)
                    && matches!(parts[2], "pulls" | "pull"),
                "PR URL must belong to the selected repository"
            );
            let number: u64 = parts[3].parse().context("PR URL must contain a number")?;
            ensure!(number > 0, "PR number must be positive");
            return Ok(number);
        }
    }
    let branch = selector
        .map(str::to_owned)
        .map_or_else(crate::repo::branch, Ok)?;
    let pulls = list(api, repo, "open", usize::MAX)?;
    let matches: Vec<_> = pulls
        .iter()
        .filter(|pull| pull.head.name == branch || pull.head.label.as_deref() == Some(&branch))
        .collect();
    ensure!(
        matches.len() == 1,
        "expected one open PR for branch {branch}, found {}; specify a PR number",
        matches.len()
    );
    Ok(matches[0].number)
}

pub fn statuses(api: &Api, repo: &Repo, sha: &str) -> Result<Vec<Status>> {
    let mut path = repo.path().to_vec();
    path.extend(["statuses", sha]);
    let statuses: Vec<Status> = api.list(&path, &[], usize::MAX)?;
    let mut latest = BTreeMap::<String, Status>::new();
    for status in statuses {
        if latest
            .get(&status.context)
            .is_none_or(|old| old.id < status.id)
        {
            latest.insert(status.context.clone(), status);
        }
    }
    Ok(latest.into_values().collect())
}
