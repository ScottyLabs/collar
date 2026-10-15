mod render;

use render::{describe_diff, describe_pull, describe_status};

use std::{
    error::Error,
    fmt,
    io::{self, IsTerminal},
};

use anyhow::{Context, Result, bail};
use console::{Style, Term, measure_text_width};

use crate::{client::Api, output::Output, pr, repo::Repo};
use dialoguer::{FuzzySelect, theme::ColorfulTheme};

#[derive(Debug)]
pub struct Cancelled;

impl fmt::Display for Cancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("selection cancelled")
    }
}

impl Error for Cancelled {}

pub fn require_terminal() -> Result<()> {
    if !io::stdin().is_terminal() || !io::stderr().is_terminal() {
        bail!("interactive selection requires terminal stdin and stderr");
    }
    Ok(())
}

const HINT: &str = "Type to filter  |  arrows move  |  Enter select  |  Esc cancel";

/// Rows taken by the blank line and the hint once wrapped to `columns`.
fn hint_rows(columns: u16) -> usize {
    1 + measure_text_width(HINT).div_ceil(usize::from(columns).max(1))
}

pub fn choose(prompt: &str, labels: &[String]) -> Result<usize> {
    if labels.is_empty() {
        bail!("cannot select from an empty list");
    }
    require_terminal()?;

    let labels: Vec<String> = labels.iter().map(|label| single_line(label)).collect();
    let term = Term::stderr();
    let accent = Style::new().for_stderr().cyan();
    let muted = Style::new().for_stderr().dim();
    let theme = ColorfulTheme {
        prompt_prefix: accent.clone().bold().apply_to("collar".into()),
        prompt_suffix: muted.apply_to("/".into()),
        success_prefix: accent.clone().apply_to("+".into()),
        success_suffix: muted.apply_to("/".into()),
        active_item_style: accent.clone().bold(),
        active_item_prefix: accent.clone().bold().apply_to(">".into()),
        fuzzy_match_highlight_style: accent.clone().bold().underlined(),
        hint_style: muted.clone(),
        values_style: accent,
        ..ColorfulTheme::default()
    };
    let rows = hint_rows(term.size().1);
    term.write_line(&format!("\n{}", muted.apply_to(HINT)))?;
    let selection = FuzzySelect::with_theme(&theme)
        .with_prompt(single_line(prompt))
        .items(&labels)
        .default(0)
        .max_length(usize::from(term.size().0.saturating_sub(7)).clamp(3, 12))
        .report(false)
        .interact_on_opt(&term)?;
    // The picker erases its own lines; erase the hint too so that repeated
    // pickers do not leave one stale copy each in the scrollback.
    term.clear_last_lines(rows)?;
    match selection {
        Some(index) => Ok(index),
        None => Err(Cancelled.into()),
    }
}

pub fn terminal_text(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || matches!(character, '\n' | '\t'))
        .collect()
}

fn single_line(text: &str) -> String {
    terminal_text(text)
        .chars()
        .map(|character| {
            if matches!(character, '\n' | '\t') {
                ' '
            } else {
                character
            }
        })
        .collect()
}

enum Action {
    Details,
    Diff,
    Checks,
    Back,
    Done,
}

impl fmt::Display for Action {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Details => "Details",
            Self::Diff => "Diff",
            Self::Checks => "Checks",
            Self::Back => "Back",
            Self::Done => "Done",
        })
    }
}

pub fn browse_pull_requests(
    api: &Api,
    repo: &Repo,
    state: &str,
    limit: usize,
    output: &Output,
) -> Result<()> {
    require_terminal()?;
    output.validate_fields(pr::Pull::FIELDS)?;
    let pulls = pr::list(api, repo, state, limit)?;
    if pulls.is_empty() {
        bail!("no pull requests match the selected state");
    }
    let labels: Vec<_> = pulls
        .iter()
        .map(|pull| {
            format!(
                "#{} {} [{}]{}",
                pull.number,
                pull.title,
                pull.user.login,
                if pull.draft { " [DRAFT]" } else { "" }
            )
        })
        .collect();
    loop {
        let number = pulls
            .get(choose("Pull request", &labels)?)
            .context("pull request selection is out of range")?
            .number;
        if output.json.is_some() {
            let pull = pr::get(api, repo, number)?;
            return output.print(&pull.output(), || eprintln!("{}", describe_pull(&pull)));
        }
        if matches!(inspect_pull_request(api, repo, number)?, Navigation::Done) {
            return Ok(());
        }
    }
}

pub enum Navigation {
    Back,
    Done,
}

pub fn inspect_pull_request(api: &Api, repo: &Repo, number: u64) -> Result<Navigation> {
    require_terminal()?;
    let actions = [
        Action::Details,
        Action::Diff,
        Action::Checks,
        Action::Back,
        Action::Done,
    ];
    let labels = actions.iter().map(ToString::to_string).collect::<Vec<_>>();
    loop {
        let action = actions
            .get(choose(&format!("Inspect PR #{number}"), &labels)?)
            .context("review action selection is out of range")?;
        match action {
            Action::Details => eprintln!("{}", describe_pull(&pr::get(api, repo, number)?)),
            Action::Diff => eprintln!("{}", describe_diff(&pr::diff(api, repo, number)?)),
            Action::Checks => {
                let pull = pr::get(api, repo, number)?;
                let statuses = pr::statuses(api, repo, &pull.head.sha)?;
                eprintln!("\nCommit {}", terminal_text(&pull.head.sha));
                if statuses.is_empty() {
                    eprintln!(
                        "No statuses reported; required checks and merge readiness are unknown."
                    );
                }
                for status in statuses {
                    eprintln!("{}", describe_status(&status));
                }
            }
            Action::Back => return Ok(Navigation::Back),
            Action::Done => return Ok(Navigation::Done),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HINT, choose, hint_rows, single_line, terminal_text};
    use console::measure_text_width;

    #[test]
    fn inspected_text_keeps_layout_but_drops_controls() {
        assert_eq!(
            terminal_text("one\ntwo\t\x1b[31mred\x07"),
            "one\ntwo\t[31mred"
        );
    }

    #[test]
    fn chooser_labels_are_single_line_and_have_no_escape_controls() {
        assert_eq!(
            single_line("one\ntwo\t\x1b]0;title\x07"),
            "one two ]0;title"
        );
    }

    #[test]
    fn hint_rows_count_the_blank_line_and_any_wrapping() {
        let width = u16::try_from(measure_text_width(HINT)).unwrap();
        assert_eq!(hint_rows(width), 2);
        assert_eq!(hint_rows(width + 40), 2);
        assert_eq!(hint_rows(width - 1), 3);
        assert_eq!(hint_rows(width / 2), 3);
        assert_eq!(hint_rows(0), 1 + measure_text_width(HINT));
    }

    #[test]
    fn empty_choices_are_rejected() {
        assert!(choose("Choose", &[]).is_err());
    }
}
