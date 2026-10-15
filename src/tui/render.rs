//! Terminal-safe inspection views, styled only after remote text is sanitized.

use console::Style;

use super::{single_line, terminal_text};
use crate::{
    diff::{self, LineKind},
    pr,
};

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
    render_diff(
        &terminal_text(diff),
        &DiffStyles::new(&Style::new().for_stderr()),
    )
}

struct DiffStyles {
    plain: Style,
    heading: Style,
    muted: Style,
    hunk: Style,
    added: Style,
    removed: Style,
}

impl DiffStyles {
    fn new(base: &Style) -> Self {
        Self {
            plain: base.clone(),
            heading: base.clone().bold(),
            muted: base.clone().dim(),
            hunk: base.clone().cyan(),
            added: base.clone().green(),
            removed: base.clone().red(),
        }
    }

    fn of(&self, kind: LineKind) -> &Style {
        match kind {
            LineKind::FileHeader | LineKind::FilePath => &self.heading,
            LineKind::Meta | LineKind::Note => &self.muted,
            LineKind::HunkHeader => &self.hunk,
            LineKind::Added => &self.added,
            LineKind::Removed => &self.removed,
            LineKind::Context => &self.plain,
        }
    }
}

/// Styles sanitized diff text; file headers and paths stay legible while
/// modes and hashes recede, and files are separated by a blank line.
fn render_diff(text: &str, styles: &DiffStyles) -> String {
    if text.trim().is_empty() {
        return format!("\n{}", styles.muted.apply_to("No changes."));
    }
    let mut rows = Vec::new();
    for (index, (line, kind)) in text.lines().zip(diff::line_kinds(text)).enumerate() {
        if kind == LineKind::FileHeader && index > 0 {
            rows.push(String::new());
        }
        rows.push(styles.of(kind).apply_to(line).to_string());
    }
    format!("\n{}", rows.join("\n"))
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

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "diff --git a/a b/a\nindex 1..2 100644\n--- a/a\n+++ b/a\n@@ -1,2 +1,2 @@\n--- old\n+++ new\n keep\ndiff --git a/b b/b\nnew file mode 100644\n";

    fn styles(forced: bool) -> DiffStyles {
        DiffStyles::new(&Style::new().force_styling(forced))
    }

    #[test]
    fn files_are_separated_and_plain_text_is_unchanged() {
        let expected = "\ndiff --git a/a b/a\nindex 1..2 100644\n--- a/a\n+++ b/a\n@@ -1,2 +1,2 @@\n--- old\n+++ new\n keep\n\ndiff --git a/b b/b\nnew file mode 100644";
        assert_eq!(render_diff(DIFF, &styles(false)), expected);
    }

    #[test]
    fn each_kind_of_line_gets_its_own_style() {
        let rendered = render_diff(DIFF, &styles(true));
        let line = |text: &str| {
            rendered
                .lines()
                .find(|line| line.contains(text))
                .unwrap()
                .to_owned()
        };
        assert!(line("diff --git a/a").starts_with("\x1b[1m"));
        assert!(line("+++ b/a").starts_with("\x1b[1m"));
        assert!(line("index 1..2").starts_with("\x1b[2m"));
        assert!(line("@@ -1,2").starts_with("\x1b[36m"));
        assert!(line("--- old").starts_with("\x1b[31m"));
        assert!(line("+++ new").starts_with("\x1b[32m"));
        assert!(line(" keep").starts_with(" keep"));
    }

    #[test]
    fn an_empty_diff_says_so() {
        assert_eq!(render_diff("", &styles(false)), "\nNo changes.");
        assert_eq!(render_diff("\n\n", &styles(false)), "\nNo changes.");
    }

    #[test]
    fn remote_escape_sequences_never_reach_the_terminal() {
        let rendered = describe_diff("@@ -1 +1 @@\n-a\n+b\x1b[35m\x1b]0;title\x07\n");
        assert!(!rendered.contains("\x1b]"));
        assert!(!rendered.contains("\x1b[35m"));
    }
}
