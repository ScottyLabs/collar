use std::{
    error::Error,
    fmt,
    io::{self, IsTerminal},
};

use anyhow::{Result, bail};
use console::Term;
use dialoguer::{FuzzySelect, theme::SimpleTheme};

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

pub fn choose(prompt: &str, labels: &[String]) -> Result<usize> {
    if labels.is_empty() {
        bail!("cannot select from an empty list");
    }
    require_terminal()?;

    let labels: Vec<String> = labels.iter().map(|label| single_line(label)).collect();
    match FuzzySelect::with_theme(&SimpleTheme)
        .with_prompt(format!(
            "{} [type: filter; Up/Down: move; Enter: select; Esc: cancel]",
            single_line(prompt)
        ))
        .items(&labels)
        .default(0)
        .max_length(8)
        .interact_on_opt(&Term::stderr())?
    {
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

pub fn browse_pull_requests(
    api: &crate::client::Api,
    repo: &crate::repo::Repo,
    state: &str,
    limit: usize,
    output: &crate::output::Output,
) -> Result<()> {
    require_terminal()?;
    output.validate_fields(crate::pr::Pull::FIELDS)?;
    let pulls = crate::pr::list(api, repo, state, limit)?;
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
    let number = pulls[choose("Pull request", &labels)?].number;
    if output.json.is_some() {
        let pull = crate::pr::get(api, repo, number)?;
        return output.print(&pull.output(), || eprintln!("{}", describe_pull(&pull)));
    }
    let actions: Vec<_> = ["Details", "Diff", "Checks", "Done"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    loop {
        let pull = crate::pr::get(api, repo, number)?;
        match choose(&format!("Inspect PR #{number}"), &actions)? {
            0 => eprintln!("{}", describe_pull(&pull)),
            1 => eprintln!("{}", terminal_text(&crate::pr::diff(api, repo, number)?)),
            2 => {
                let statuses = crate::pr::statuses(api, repo, &pull.head.sha)?;
                eprintln!("Commit {}", terminal_text(&pull.head.sha));
                if statuses.is_empty() {
                    eprintln!(
                        "No statuses reported; required checks and merge readiness are unknown."
                    );
                }
                for status in statuses {
                    eprintln!(
                        "{}",
                        terminal_text(&format!(
                            "{}\t{}\t{}",
                            status.context,
                            status.state,
                            status.target_url.as_deref().unwrap_or("")
                        ))
                    );
                }
            }
            3 => return Ok(()),
            _ => unreachable!("selector returns a valid action index"),
        }
    }
}

fn describe_pull(pull: &crate::pr::Pull) -> String {
    terminal_text(&format!(
        "#{} {}\n{} -> {}\nState: {}\nCommit: {}\n{}\n\n{}",
        pull.number,
        pull.title,
        pull.head.name,
        pull.base.name,
        if pull.merged { "merged" } else { &pull.state },
        pull.head.sha,
        pull.html_url,
        pull.body.as_deref().unwrap_or("")
    ))
}

#[cfg(test)]
mod tests {
    use super::{choose, single_line, terminal_text};

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
    fn empty_choices_are_rejected() {
        assert!(choose("Choose", &[]).is_err());
    }
}
