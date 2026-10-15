use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;

use crate::output::Output;

#[derive(Args)]
pub(super) struct Selection {
    /// PR number, same-repository URL, or branch; defaults to the current branch.
    pub(super) selector: Option<String>,
    #[command(flatten)]
    pub(super) output: Output,
}

#[derive(Args)]
pub(super) struct PullText {
    /// Markdown text for the description, discussion comment, or review.
    #[arg(short = 'b', long, conflicts_with = "body_file")]
    pub(super) body: Option<String>,
    /// Read Markdown from a file, or - for stdin.
    #[arg(short = 'F', long)]
    pub(super) body_file: Option<PathBuf>,
}

impl PullText {
    pub(super) fn read(self) -> Result<Option<String>> {
        self.body_file
            .map(|path| read_file(&path))
            .transpose()
            .map(|file| file.or(self.body))
    }
}
pub(super) fn read_file(path: &Path) -> Result<String> {
    if path == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .context("read stdin")?;
        Ok(text)
    } else {
        std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))
    }
}
