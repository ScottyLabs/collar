use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;

use crate::{
    output::Output,
    source::{Kind, Sources},
};

#[derive(Subcommand)]
pub enum Command {
    /// List available Markdown topics.
    List(SourceArgs),
    /// Read a Markdown topic.
    View {
        topic: String,
        #[command(flatten)]
        source: SourceArgs,
    },
    /// Search Markdown topics for a literal phrase.
    Search {
        query: String,
        #[command(flatten)]
        source: SourceArgs,
    },
}

#[derive(Args)]
pub struct SourceArgs {
    /// Documentation service to read.
    #[arg(long, value_enum, default_value = "scottylabs")]
    service: Service,
    /// Read a local service checkout instead of a canonical snapshot.
    #[arg(long)]
    checkout: Option<PathBuf>,
    /// Read the last verified canonical snapshot without network access.
    #[arg(long)]
    offline: bool,
    #[command(flatten)]
    output: Output,
}

#[derive(Clone, Copy, Debug, PartialEq, ValueEnum)]
pub enum Service {
    Scottylabs,
    Kennel,
    Infrastructure,
    Governance,
}

impl Service {
    fn kind(self) -> Kind {
        match self {
            Self::Scottylabs => Kind::Documentation,
            Self::Kennel => Kind::Kennel,
            Self::Infrastructure => Kind::Infrastructure,
            Self::Governance => Kind::Governance,
        }
    }

    fn docs_root(self) -> &'static str {
        match self {
            Self::Scottylabs => "org-docs/scottylabs",
            Self::Kennel => "sites/docs/src",
            Self::Infrastructure => "docs/src",
            Self::Governance => "",
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Topic {
    topic: String,
    path: String,
    title: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document {
    topic: String,
    path: String,
    title: String,
    body: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Match {
    topic: String,
    path: String,
    title: String,
    snippet: String,
}

pub fn run(command: Command, sources: &Sources<'_>) -> Result<()> {
    let (args, action) = match command {
        Command::List(args) => (args, Action::List),
        Command::View { topic, source } => (source, Action::View(topic)),
        Command::Search { query, source } => (source, Action::Search(query)),
    };
    let service = args.service;
    match &action {
        Action::List => args
            .output
            .validate_fields(&["topic", "path", "title", "dataSource"])?,
        Action::View(_) => {
            args.output
                .validate_fields(&["topic", "path", "title", "body", "dataSource"])?
        }
        Action::Search(_) => {
            args.output
                .validate_fields(&["topic", "path", "title", "snippet", "dataSource"])?
        }
    }
    let source = sources.load(service.kind(), args.checkout, args.offline)?;
    match action {
        Action::List => {
            let topics = list_topics(&source.directory, service)?;
            args.output.print(&source.attach(&topics)?, || {
                crate::ui::heading("Documentation topics");
                crate::ui::detail("Source", &source.describe());
                crate::ui::table(
                    &["Topic", "Title"],
                    &topics
                        .iter()
                        .map(|topic| vec![topic.topic.clone(), topic.title.clone()])
                        .collect::<Vec<_>>(),
                );
            })
        }
        Action::View(topic) => {
            let doc = view_topic(&source.directory, service, &topic)?;
            args.output.print(&source.attach(&doc)?, || {
                crate::ui::heading(&doc.title);
                crate::ui::detail("Source", &format!("{} ({})", doc.path, source.describe()));
                crate::ui::markdown(&doc.body);
            })
        }
        Action::Search(query) => {
            let matches = search(&source.directory, service, &query)?;
            args.output.print(&source.attach(&matches)?, || {
                crate::ui::heading("Documentation search results");
                crate::ui::detail("Source", &source.describe());
                crate::ui::table(
                    &["Topic", "Path", "Match"],
                    &matches
                        .iter()
                        .map(|result| {
                            vec![
                                result.topic.clone(),
                                result.path.clone(),
                                result.snippet.clone(),
                            ]
                        })
                        .collect::<Vec<_>>(),
                );
            })
        }
    }
}

enum Action {
    List,
    View(String),
    Search(String),
}

fn list_topics(checkout: &Path, service: Service) -> Result<Vec<Topic>> {
    let files = markdown_files(checkout, service)?;
    files
        .into_iter()
        .map(|path| {
            let body = std::fs::read_to_string(checkout.join(&path))
                .with_context(|| format!("read documentation {}", path.display()))?;
            let path_string = slash_path(&path);
            Ok(Topic {
                topic: topic_from_path(service, &path),
                path: path_string.clone(),
                title: title(&body, &path_string),
            })
        })
        .collect()
}

fn view_topic(checkout: &Path, service: Service, requested: &str) -> Result<Document> {
    ensure!(
        safe_topic(requested),
        "topic must be a safe relative Markdown topic"
    );
    let requested = requested
        .strip_suffix(".md")
        .or_else(|| requested.strip_suffix(".mdx"))
        .unwrap_or(requested);
    let path = markdown_files(checkout, service)?
        .into_iter()
        .find(|path| topic_from_path(service, path) == requested)
        .with_context(|| {
            format!("documentation topic {requested:?} was not found for {service:?}")
        })?;
    let path_string = slash_path(&path);
    let body = std::fs::read_to_string(checkout.join(&path))
        .with_context(|| format!("read documentation {}", path.display()))?;
    Ok(Document {
        topic: topic_from_path(service, &path),
        title: title(&body, &path_string),
        path: path_string,
        body,
    })
}

fn search(checkout: &Path, service: Service, query: &str) -> Result<Vec<Match>> {
    ensure!(!query.trim().is_empty(), "search query must not be empty");
    let needle = query.to_lowercase();
    let mut result = Vec::new();
    for path in markdown_files(checkout, service)? {
        let path_string = slash_path(&path);
        let body = std::fs::read_to_string(checkout.join(&path))
            .with_context(|| format!("read documentation {}", path.display()))?;
        if let Some(line) = body
            .lines()
            .find(|line| line.to_lowercase().contains(&needle))
        {
            result.push(Match {
                topic: topic_from_path(service, &path),
                path: path_string.clone(),
                title: title(&body, &path_string),
                snippet: truncate(line.trim(), 240),
            });
        }
    }
    Ok(result)
}

fn markdown_files(checkout: &Path, service: Service) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    match service {
        Service::Governance => {
            let path = checkout.join("README.md");
            ensure!(
                path.symlink_metadata()
                    .is_ok_and(|meta| meta.file_type().is_file()),
                "governance checkout is missing README.md"
            );
            files.push(PathBuf::from("README.md"));
        }
        Service::Scottylabs => {
            collect_markdown(&checkout.join(service.docs_root()), checkout, &mut files)?
        }
        Service::Kennel => {
            let readme = checkout.join("README.md");
            ensure!(
                readme
                    .symlink_metadata()
                    .is_ok_and(|meta| meta.file_type().is_file()),
                "Kennel checkout is missing README.md"
            );
            files.push(PathBuf::from("README.md"));
            collect_markdown(&checkout.join(service.docs_root()), checkout, &mut files)?;
        }
        Service::Infrastructure => {
            collect_markdown(&checkout.join(service.docs_root()), checkout, &mut files)?
        }
    }
    files.sort();
    ensure!(
        files.len() <= 1000,
        "documentation checkout has more than 1000 Markdown files"
    );
    let mut total = 0u64;
    for path in &files {
        let size = checkout
            .join(path)
            .metadata()
            .with_context(|| format!("inspect documentation {}", path.display()))?
            .len();
        ensure!(
            size <= 1024 * 1024,
            "documentation file {} exceeds 1 MiB",
            path.display()
        );
        total = total.saturating_add(size);
    }
    ensure!(
        total <= 8 * 1024 * 1024,
        "documentation source exceeds 8 MiB"
    );
    Ok(files)
}

fn collect_markdown(directory: &Path, checkout: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    ensure!(
        directory.is_dir(),
        "documentation directory {} is missing",
        directory.display()
    );
    for entry in std::fs::read_dir(directory)
        .with_context(|| format!("read documentation directory {}", directory.display()))?
    {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            collect_markdown(&entry.path(), checkout, files)?;
        } else if ty.is_file()
            && entry
                .path()
                .extension()
                .is_some_and(|ext| ext == "md" || ext == "mdx")
        {
            let path = entry
                .path()
                .strip_prefix(checkout)
                .context("documentation path escaped checkout")?
                .to_path_buf();
            if !path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| {
                    name.eq_ignore_ascii_case("SUMMARY.md")
                        || name.eq_ignore_ascii_case("SUMMARY.mdx")
                })
            {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn topic_from_path(service: Service, path: &Path) -> String {
    if path == Path::new("README.md") {
        return "README".into();
    }
    let relative = if service == Service::Scottylabs
        || service == Service::Kennel
        || service == Service::Infrastructure
    {
        path.strip_prefix(service.docs_root()).unwrap_or(path)
    } else {
        path
    };
    let mut topic = slash_path(relative);
    if let Some(stem) = topic
        .strip_suffix(".md")
        .or_else(|| topic.strip_suffix(".mdx"))
    {
        topic = stem.to_owned();
    }
    topic
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn safe_topic(topic: &str) -> bool {
    !topic.is_empty()
        && !topic.starts_with('/')
        && !topic.contains(['\\', '\0'])
        && !topic.chars().any(char::is_control)
        && topic
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != ".." && !part.contains(':'))
}

fn title(body: &str, fallback: &str) -> String {
    if let Some(metadata) = body
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(metadata, _)| metadata))
    {
        for line in metadata.lines() {
            if let Some(title) = line.strip_prefix("title:") {
                let title = title.trim();
                let title = title
                    .strip_prefix('"')
                    .and_then(|value| value.strip_suffix('"'))
                    .or_else(|| {
                        title
                            .strip_prefix('\'')
                            .and_then(|value| value.strip_suffix('\''))
                    })
                    .unwrap_or(title);
                if !title.is_empty() {
                    return title.to_owned();
                }
            }
        }
    }
    body.lines()
        .find_map(|line| line.strip_prefix("# "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

fn truncate(line: &str, limit: usize) -> String {
    let mut chars = line.chars();
    let text: String = chars.by_ref().take(limit).collect();
    if chars.next().is_some() {
        format!("{text}...")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starlight_frontmatter_supplies_the_display_title() {
        assert_eq!(
            title("---\ntitle: Communication\n---\n\n## Join us\n", "page.md"),
            "Communication"
        );
        assert_eq!(
            title("---\ntitle: 'Getting Started'\n---\n", "page.md"),
            "Getting Started"
        );
    }

    #[test]
    fn topics_are_discovered_without_summary_and_search_is_literal_case_insensitive() {
        let temp = tempfile::tempdir().unwrap();
        let docs = temp.path().join("sites/docs/src/guides");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(temp.path().join("README.md"), "# Overview\n").unwrap();
        std::fs::write(temp.path().join("sites/docs/src/SUMMARY.md"), "not a topic").unwrap();
        std::fs::write(
            docs.join("features.md"),
            "# Features\nUse `kennel` for builds.\n",
        )
        .unwrap();
        let topics = list_topics(temp.path(), Service::Kennel).unwrap();
        assert_eq!(
            topics
                .iter()
                .map(|topic| topic.topic.as_str())
                .collect::<Vec<_>>(),
            ["README", "guides/features"]
        );
        assert_eq!(
            search(temp.path(), Service::Kennel, "KENNEL").unwrap()[0].snippet,
            "Use `kennel` for builds."
        );
        assert!(view_topic(temp.path(), Service::Kennel, "../README").is_err());
    }
}
