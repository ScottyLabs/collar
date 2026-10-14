use std::{fs, path::PathBuf, process};

use anyhow::{Context, Result, ensure};
use clap::{Args, Subcommand, ValueEnum};
use serde_json::{Value, json};

use crate::{
    output::Output,
    source::{Kind, Sources},
};

#[derive(Subcommand)]
pub enum Command {
    /// Read the canonical generated summary for a DevOps repository.
    View {
        #[arg(value_enum)]
        repository: Repository,
        #[command(flatten)]
        source: SourceArgs,
    },
    /// List numbered tech-lead announcements, newest first.
    Updates {
        #[command(flatten)]
        source: SourceArgs,
    },
    /// Read a numbered tech-lead announcement.
    Update {
        number: u32,
        #[command(flatten)]
        source: SourceArgs,
    },
    /// Invoke the canonical authoring helper in an explicit working checkout.
    New {
        slug: String,
        #[arg(long)]
        checkout: PathBuf,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum Repository {
    Kennel,
    Governance,
    Infrastructure,
}

impl Repository {
    fn file(self) -> &'static str {
        match self {
            Self::Kennel => "kennel.md",
            Self::Governance => "governance.md",
            Self::Infrastructure => "infrastructure.md",
        }
    }
}

#[derive(Args)]
pub struct SourceArgs {
    /// Inspect local changes instead of the latest canonical snapshot.
    #[arg(long)]
    checkout: Option<PathBuf>,
    /// Read the last verified snapshot without network access.
    #[arg(long)]
    offline: bool,
    #[command(flatten)]
    output: Output,
}

pub fn update_number(name: &str) -> Option<u32> {
    let (number, slug) = name.strip_suffix(".md")?.split_once('-')?;
    if number.len() < 3 || !number.bytes().all(|byte| byte.is_ascii_digit()) || !valid_slug(slug) {
        return None;
    }
    number.parse().ok()
}

fn valid_slug(slug: &str) -> bool {
    slug.split('-').all(|part| {
        !part.is_empty()
            && part
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
    })
}

pub fn run(command: Command, sources: &Sources<'_>) -> Result<()> {
    let (args, file, number) = match command {
        Command::View { repository, source } => (source, Some(repository.file()), None),
        Command::Updates { source } => (source, None, None),
        Command::Update { number, source } => (source, None, Some(number)),
        Command::New { slug, checkout } => {
            ensure!(
                valid_slug(&slug),
                "slug must be lowercase words joined by hyphens"
            );
            let checkout = checkout.canonicalize().context("find changelog checkout")?;
            ensure!(
                checkout.join("new-update.sh").is_file() && checkout.join("updates").is_dir(),
                "checkout must contain the changelog authoring helper and updates directory"
            );
            return crate::process::run(
                process::Command::new("bash")
                    .current_dir(&checkout)
                    .arg(checkout.join("new-update.sh"))
                    .arg(slug),
            );
        }
    };
    args.output
        .validate_fields(&["path", "title", "body", "number", "dataSource"])?;
    let source = sources.load(Kind::Changelog, args.checkout, args.offline)?;
    let read = |path: &str, number: Option<u32>| -> Result<Value> {
        let body = fs::read_to_string(source.directory.join(path))
            .with_context(|| format!("read changelog {path}"))?;
        let title = body
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .unwrap_or(path);
        Ok(json!({"path":path,"number":number,"title":title,"body":body}))
    };
    if let Some(file) = file {
        let value = source.attach(&read(file, None)?)?;
        return args.output.print(&value, || {
            println!("{}\n{}", source.describe(), value["body"].as_str().unwrap());
        });
    }
    let mut updates = Vec::new();
    for entry in fs::read_dir(source.directory.join("updates")).context("read changelog updates")? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if let Some(number) = update_number(name) {
            ensure!(
                entry.file_type()?.is_file(),
                "announcement must be a regular file"
            );
            updates.push((number, format!("updates/{name}")));
        }
    }
    updates.sort_by_key(|update| std::cmp::Reverse(update.0));
    ensure!(
        !updates.windows(2).any(|pair| pair[0].0 == pair[1].0),
        "duplicate announcement number"
    );
    if let Some(number) = number {
        let (_, path) = updates
            .iter()
            .find(|(candidate, _)| *candidate == number)
            .with_context(|| format!("announcement {number} does not exist"))?;
        let value = source.attach(&read(path, Some(number))?)?;
        return args.output.print(&value, || {
            println!("{}\n{}", source.describe(), value["body"].as_str().unwrap())
        });
    }
    let values: Vec<_> = updates
        .iter()
        .map(|(number, path)| read(path, Some(*number)))
        .collect::<Result<_>>()?;
    args.output.print(&source.attach(&values)?, || {
        println!("{}", source.describe());
        for value in &values {
            println!(
                "{}\t{}",
                value["path"].as_str().unwrap(),
                value["title"].as_str().unwrap()
            );
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_numbered_update_paths_and_canonical_slugs() {
        assert_eq!(update_number("017-ci-secrets-per-project.md"), Some(17));
        for name in [
            "README.md",
            "17-short.md",
            "017--bad.md",
            "017-upperCase.md",
            "017-../bad.md",
        ] {
            assert_eq!(update_number(name), None);
        }
        assert!(!valid_slug(""));
        assert!(!valid_slug("--help"));
    }
}
