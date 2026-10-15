//! Terminal-safe inspection views, styled only after remote text is sanitized.

use console::Style;

use super::{single_line, terminal_text};
use crate::pr;

pub(super) fn describe_pull(pull: &pr::Pull) -> String {
    let muted = Style::new().for_stderr().dim();
    let detail = |label: &str, value: String| {
        format!("  {}  {value}", muted.apply_to(format!("{label:<6}")))
    };
    format!(
        "\n{}\n\n{}\n{}\n{}\n{}\n\n{}\n",
        Style::new().for_stderr().bold().apply_to(format!(
            "#{} {}",
            pull.number,
            single_line(&pull.title)
        )),
        detail(
            "State",
            single_line(if pull.merged { "merged" } else { &pull.state })
        ),
        detail(
            "Branch",
            format!(
                "{} -> {}",
                single_line(&pull.head.name),
                single_line(&pull.base.name)
            )
        ),
        detail("Commit", single_line(&pull.head.sha)),
        detail("URL", single_line(&pull.html_url)),
        terminal_text(pull.body.as_deref().unwrap_or("")),
    )
}

pub(super) fn describe_diff(diff: &str) -> String {
    terminal_text(diff)
        .lines()
        .map(|line| {
            let style = Style::new().for_stderr();
            let style = if line.starts_with("---")
                || line.starts_with("+++")
                || line.starts_with("diff ")
                || line.starts_with("index ")
            {
                style.dim()
            } else if line.starts_with('+') {
                style.green()
            } else if line.starts_with('-') {
                style.red()
            } else if line.starts_with("@@") {
                style.cyan()
            } else {
                style
            };
            style.apply_to(line).to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn describe_status(status: &pr::Status) -> String {
    let style = Style::new().for_stderr();
    let state = match status.state.as_str() {
        "success" => style.green(),
        "error" | "failure" => style.red(),
        "pending" => style.yellow(),
        _ => style.dim(),
    };
    format!(
        "  {}  {}\n    {}",
        state.apply_to(single_line(&status.state)),
        Style::new()
            .for_stderr()
            .bold()
            .apply_to(single_line(&status.context)),
        Style::new()
            .for_stderr()
            .dim()
            .apply_to(single_line(status.target_url.as_deref().unwrap_or(""))),
    )
}
