//! Parses unified Git diffs to validate inline review anchors on either side
//! and to classify lines for display.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};

struct Range {
    next: u64,
    remaining: u64,
}

impl Range {
    fn parse(value: &str, prefix: char) -> Result<Self> {
        let value = value.strip_prefix(prefix).context("invalid diff range")?;
        let (start, count) = value.split_once(',').unwrap_or((value, "1"));
        let next: u64 = start.parse().context("invalid diff start")?;
        let remaining: u64 = count.parse().context("invalid diff count")?;
        ensure!(
            remaining == 0 || next > 0,
            "nonempty diff range must start at a positive line"
        );
        next.checked_add(remaining)
            .context("diff line range overflows")?;
        Ok(Self { next, remaining })
    }

    fn take(&mut self) -> Result<u64> {
        ensure!(
            self.remaining > 0,
            "diff content exceeds its hunk's line count"
        );
        let line = self.next;
        self.next += 1;
        self.remaining -= 1;
        Ok(line)
    }

    fn skip(&mut self) {
        self.remaining = self.remaining.saturating_sub(1);
    }
}

struct Hunk {
    old: Range,
    new: Range,
}

impl Hunk {
    fn complete(&self) -> bool {
        self.old.remaining == 0 && self.new.remaining == 0
    }

    fn parse(header: &str) -> Result<Self> {
        let mut parts = header.splitn(3, ' ');
        let old = Range::parse(parts.next().context("missing old diff range")?, '-')?;
        let new = Range::parse(parts.next().context("missing new diff range")?, '+')?;
        ensure!(
            parts.next().is_some_and(|part| part.starts_with("@@")),
            "invalid diff hunk terminator"
        );
        Ok(Self { old, new })
    }
}

/// How one line of a unified diff is presented.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineKind {
    /// The `diff --git` line that starts a file.
    FileHeader,
    /// An old or new `---` / `+++` path header.
    FilePath,
    /// Other extended headers: modes, index, renames, binary notices.
    Meta,
    /// An `@@` hunk header.
    HunkHeader,
    Added,
    Removed,
    Context,
    /// The `\ No newline at end of file` marker.
    Note,
}

/// Classifies each line of `diff` for display, in order.
///
/// Unlike [`anchors`] this never fails, so any diff can still be shown. Hunk
/// line counts decide where content ends, which keeps a removed line such as
/// `--- text` from being read as a file header. A truncated hunk cannot swallow
/// the next file because `diff ` and `@@ ` never begin hunk content.
pub fn line_kinds(diff: &str) -> Vec<LineKind> {
    let mut open: Option<Hunk> = None;
    diff.lines()
        .map(|line| {
            if line.starts_with('\\') {
                return LineKind::Note;
            }
            if line.starts_with("diff ") {
                open = None;
                return LineKind::FileHeader;
            }
            if let Some(header) = line.strip_prefix("@@ ") {
                open = Hunk::parse(header).ok();
                return LineKind::HunkHeader;
            }
            if let Some(hunk) = open.as_mut().filter(|hunk| !hunk.complete()) {
                return match line.as_bytes().first() {
                    Some(b'+') => {
                        hunk.new.skip();
                        LineKind::Added
                    }
                    Some(b'-') => {
                        hunk.old.skip();
                        LineKind::Removed
                    }
                    _ => {
                        hunk.old.skip();
                        hunk.new.skip();
                        LineKind::Context
                    }
                };
            }
            if line.starts_with("--- ") || line.starts_with("+++ ") {
                LineKind::FilePath
            } else {
                LineKind::Meta
            }
        })
        .collect()
}

pub fn anchors(diff: &str) -> Result<BTreeSet<(String, bool, u64)>> {
    let mut anchors = BTreeSet::new();
    let mut old_path: Option<String> = None;
    let mut new_path: Option<String> = None;
    let mut old_header = false;
    let mut new_header = false;
    let mut hunk: Option<Hunk> = None;
    for line in diff.lines() {
        if line == "\\ No newline at end of file" {
            continue;
        }
        if let Some(hunk) = &mut hunk
            && !hunk.complete()
        {
            let path = new_path
                .as_ref()
                .or(old_path.as_ref())
                .context("diff hunk has no file path")?;
            match line.as_bytes().first() {
                Some(b' ') => {
                    anchors.insert((path.clone(), false, hunk.old.take()?));
                    anchors.insert((path.clone(), true, hunk.new.take()?));
                }
                Some(b'-') => {
                    anchors.insert((path.clone(), false, hunk.old.take()?));
                }
                Some(b'+') => {
                    anchors.insert((path.clone(), true, hunk.new.take()?));
                }
                _ => bail!("diff hunk is truncated or contains an invalid line"),
            }
            continue;
        }
        if line.starts_with("diff --git ") {
            old_path = None;
            new_path = None;
            old_header = false;
            new_header = false;
            hunk = None;
        } else if let Some(header) = line.strip_prefix("@@ ") {
            ensure!(
                old_header && new_header && (old_path.is_some() || new_path.is_some()),
                "diff hunk requires complete old and new file headers"
            );
            hunk = Some(Hunk::parse(header)?);
        } else if hunk.is_none()
            && let Some(path) = line.strip_prefix("--- ")
        {
            ensure!(
                !old_header && !new_header,
                "duplicate or out-of-order old diff file header"
            );
            old_path = git_path(path, "a/")?;
            old_header = true;
        } else if hunk.is_none()
            && let Some(path) = line.strip_prefix("+++ ")
        {
            ensure!(
                old_header && !new_header,
                "duplicate or out-of-order new diff file header"
            );
            new_path = git_path(path, "b/")?;
            new_header = true;
        } else if hunk.is_some() && !line.is_empty() {
            bail!("unexpected content after a complete diff hunk");
        }
    }
    ensure!(
        hunk.as_ref().is_none_or(Hunk::complete),
        "diff ends before its hunk's line count is satisfied"
    );
    Ok(anchors)
}

fn git_path(value: &str, prefix: &str) -> Result<Option<String>> {
    if value == "/dev/null" {
        return Ok(None);
    }
    let decoded = if let Some(quoted) = value.strip_prefix('"') {
        let mut bytes = quoted.bytes();
        let mut decoded = Vec::new();
        let mut closed = false;
        while let Some(byte) = bytes.next() {
            match byte {
                b'"' => {
                    ensure!(
                        bytes.next().is_none(),
                        "unexpected text after quoted diff path"
                    );
                    closed = true;
                    break;
                }
                b'\\' => {
                    let escaped = bytes.next().context("incomplete diff path escape")?;
                    decoded.push(match escaped {
                        b'"' | b'\\' => escaped,
                        b'a' => 7,
                        b'b' => 8,
                        b't' => 9,
                        b'n' => 10,
                        b'v' => 11,
                        b'f' => 12,
                        b'r' => 13,
                        b'0'..=b'3' => {
                            let second =
                                bytes.next().context("incomplete octal diff path escape")?;
                            let third =
                                bytes.next().context("incomplete octal diff path escape")?;
                            ensure!(
                                (b'0'..=b'7').contains(&second) && (b'0'..=b'7').contains(&third),
                                "invalid octal diff path escape"
                            );
                            (escaped - b'0') * 64 + (second - b'0') * 8 + (third - b'0')
                        }
                        _ => bail!("invalid diff path escape"),
                    });
                }
                byte => decoded.push(byte),
            }
        }
        ensure!(closed, "unterminated quoted diff path");
        String::from_utf8(decoded).context("diff filename is not UTF-8")?
    } else {
        value.to_owned()
    };
    let path = decoded
        .strip_prefix(prefix)
        .context("unexpected diff filename prefix")?;
    ensure!(
        !path.is_empty() && !path.contains('\0'),
        "invalid diff filename"
    );
    Ok(Some(path.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deleted_files_and_quoted_utf8_paths_have_anchors() {
        let deleted =
            "diff --git a/gone b/gone\n--- a/gone\n+++ /dev/null\n@@ -1,2 +0,0 @@\n-one\n-two\n";
        assert_eq!(
            anchors(deleted).unwrap(),
            BTreeSet::from([("gone".into(), false, 1), ("gone".into(), false, 2)])
        );
        let quoted = "diff --git \"a/caf\\303\\251.rs\" \"b/caf\\303\\251.rs\"\n--- \"a/caf\\303\\251.rs\"\n+++ \"b/caf\\303\\251.rs\"\n@@ -1 +1 @@\n-old\n+new\n";
        assert!(
            anchors(quoted)
                .unwrap()
                .contains(&("café.rs".into(), true, 1))
        );
    }

    #[test]
    fn malformed_hunks_cannot_create_commentable_lines() {
        let header = "diff --git a/file b/file\n--- a/file\n+++ b/file\n";
        for content in [
            "@@ -1 +1 @@\n-old\n+new\n+extra\n",
            "@@ -1,2 +1,2 @@\n-old\n+new\n",
            "@@ -18446744073709551615 +1 @@\n-old\n+new\n",
            "@@ -0 +1 @@\n-old\n+new\n",
            "@@ -1 +1 nope\n-old\n+new\n",
        ] {
            assert!(
                anchors(&format!("{header}{content}")).is_err(),
                "accepted {content}"
            );
        }
        assert!(
            anchors("diff --git a/file b/file\n--- a/file\n@@ -1 +1 @@\n-old\n+new\n").is_err()
        );
    }

    #[test]
    fn invalid_quoted_paths_fail_without_overflowing() {
        for path in [
            r#""b/\377.rs""#,
            r#""b/\400.rs""#,
            r#""b/\37.rs""#,
            r#""b/unterminated"#,
        ] {
            assert!(git_path(path, "b/").is_err());
        }
        assert_eq!(
            git_path(r#""b/with\"quote\tfile.rs""#, "b/").unwrap(),
            Some("with\"quote\tfile.rs".into())
        );
    }

    #[test]
    fn classifies_a_new_file_diff() {
        use LineKind::*;
        let diff = "diff --git a/n.md b/n.md\nnew file mode 100644\nindex 0000000..6ce559d\n--- /dev/null\n+++ b/n.md\n@@ -0,0 +1,2 @@\n+one\n+two\n";
        assert_eq!(
            line_kinds(diff),
            [
                FileHeader, Meta, Meta, FilePath, FilePath, HunkHeader, Added, Added
            ]
        );
    }

    #[test]
    fn content_that_looks_like_a_header_stays_content() {
        use LineKind::*;
        let diff = "diff --git a/q.sql b/q.sql\nindex 1..2 100644\n--- a/q.sql\n+++ b/q.sql\n@@ -1,2 +1,2 @@\n--- old comment\n+++ new comment\n keep\n";
        assert_eq!(
            line_kinds(diff),
            [
                FileHeader, Meta, FilePath, FilePath, HunkHeader, Removed, Added, Context
            ]
        );
    }

    #[test]
    fn the_next_file_starts_fresh_after_complete_or_truncated_hunks() {
        use LineKind::*;
        let complete = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1 +1 @@\n-x\n+y\ndiff --git a/b b/b\n--- a/b\n+++ b/b\n";
        assert_eq!(
            line_kinds(complete),
            [
                FileHeader, FilePath, FilePath, HunkHeader, Removed, Added, FileHeader, FilePath,
                FilePath
            ]
        );
        let truncated = "diff --git a/a b/a\n--- a/a\n+++ b/a\n@@ -1,5 +1,5 @@\n-x\ndiff --git a/b b/b\n--- a/b\n";
        assert_eq!(
            line_kinds(truncated),
            [
                FileHeader, FilePath, FilePath, HunkHeader, Removed, FileHeader, FilePath
            ]
        );
    }

    #[test]
    fn marks_missing_newline_notes_and_survives_bad_headers() {
        use LineKind::*;
        let diff =
            "@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\n\\ No newline at end of file\n";
        assert_eq!(line_kinds(diff), [HunkHeader, Removed, Note, Added, Note]);
        assert_eq!(line_kinds("@@ nonsense @@\nstray\n"), [HunkHeader, Meta]);
        assert!(line_kinds("").is_empty());
    }
}
