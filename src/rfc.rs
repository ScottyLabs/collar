use anyhow::{Context, Result, ensure};
use clap::Args as ClapArgs;
use console::{style, truncate_str};
use pulldown_cmark::{Event, HeadingLevel, Parser, Tag, TagEnd};
use std::io::Write;
use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

#[derive(ClapArgs)]
#[command(
    about = "Search RFC titles and sections",
    after_help = "All query words must match within a section or its RFC title.
With no query, list RFCs. Search ignores case.
Matches are ranked by relevance (title hits, then most matching sections) before falling back to filename order."
)]
pub struct Args {
    /// Directory containing RFC Markdown files.
    #[arg(long)]
    directory: Option<PathBuf>,
    /// Status to include, or 'all' for every status
    #[arg(long, default_value = "accepted")]
    status: String,
    /// Create a new Draft RFC with this title instead of searching, and print its path
    #[arg(long, value_name = "TITLE", conflicts_with = "query")]
    new: Option<String>,
    /// Words to search for
    query: Vec<String>,
}

struct Section {
    heading: String,
    anchor: String,
    line: usize,
    text: String,
}

struct Rfc {
    title: String,
    status: String,
    updated: Option<String>,
    relations: Vec<(String, String)>,
    sections: Vec<Section>,
}

fn parse(source: &str, filename: &str) -> Rfc {
    let mut rfc = Rfc {
        title: filename.into(),
        status: "Unknown".into(),
        updated: None,
        relations: Vec::new(),
        sections: Vec::new(),
    };
    let mut headings = Vec::new();
    let mut current = None;
    for (event, range) in Parser::new(source).into_offset_iter() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level, range.start, String::new()))
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, _, heading)) = &mut current {
                    heading.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, start, heading)) = current.take() {
                    if level == HeadingLevel::H1 && rfc.title == filename {
                        rfc.title = heading.clone();
                    }
                    headings.push((start, heading));
                }
            }
            _ => {}
        }
    }
    if headings.first().is_none_or(|(start, _)| *start > 0) {
        headings.insert(0, (0, filename.into()));
    }
    let mut anchors = HashSet::new();
    for (index, (start, heading)) in headings.iter().enumerate() {
        let end = headings
            .get(index + 1)
            .map_or(source.len(), |(start, _)| *start);
        let base: String = heading
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || c.is_whitespace() || matches!(c, '_' | '-'))
            .map(|c| if c.is_whitespace() { '-' } else { c })
            .collect();
        let mut anchor = base.clone();
        let mut suffix = 1;
        while !anchors.insert(anchor.clone()) {
            anchor = format!("{base}-{suffix}");
            suffix += 1;
        }
        rfc.sections.push(Section {
            heading: heading.clone(),
            anchor,
            line: source[..*start].lines().count() + 1,
            text: source[*start..end].into(),
        });
    }
    if let Some(status) = source
        .lines()
        .find_map(|line| line.trim().strip_prefix("- **Status:**"))
    {
        rfc.status = status.trim().into();
    }
    if let Some(updated) = source
        .lines()
        .find_map(|line| line.trim().strip_prefix("- **Updated:**"))
    {
        rfc.updated = Some(updated.trim().into());
    }
    for line in source.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("- **") else {
            continue;
        };
        let Some((key, value)) = rest.split_once(":**") else {
            continue;
        };
        let normalized = key.to_lowercase();
        if normalized.contains("supersede") || normalized.contains("related") {
            rfc.relations
                .push((key.to_string(), value.trim().to_string()));
        }
    }
    rfc
}

fn slugify(title: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = true; // suppress a leading dash
    for c in title.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }
    while slug.ends_with('-') {
        slug.pop();
    }
    slug
}

fn next_number(directory: &Path) -> Result<u32> {
    let mut max = 0u32;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let filename = entry.file_name().to_string_lossy().into_owned();
        if let Some((number, name)) = filename.split_once('-')
            && number.len() == 4
            && number.bytes().all(|c| c.is_ascii_digit())
            && name.ends_with(".md")
            && entry.file_type()?.is_file()
        {
            max = max.max(number.parse::<u32>().context("parse RFC number")?);
        }
    }
    ensure!(max < 9999, "RFC numbers exhausted; maximum is 9999");
    Ok(max + 1)
}

// UTC calendar date using Howard Hinnant's civil-from-days algorithm:
// https://howardhinnant.github.io/date_algorithms.html
fn today() -> Result<String> {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("system clock is before the Unix epoch")?
        .as_secs();
    let z = (secs / 86_400) as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    Ok(format!("{y:04}-{m:02}-{d:02}"))
}

fn create_new_rfc(directory: &Path, title: &str) -> Result<PathBuf> {
    ensure!(
        !title.chars().any(char::is_control),
        "RFC title must be a single line without control characters"
    );
    let number = next_number(directory)?;
    let slug = slugify(title);
    if slug.is_empty() {
        anyhow::bail!("title must contain at least one alphanumeric character");
    }
    let filename = format!("{number:04}-{slug}.md");
    let path = directory.join(&filename);
    let today = today()?;
    let template = format!(
        "# RFC {number:04}: {title}\n\
         \n\
         - **Status:** Draft\n\
         - **Author(s):** @username\n\
         - **Created:** {today}\n\
         - **Updated:** {today}\n\
         \n\
         ## Overview\n\
         \n\
         Brief 2-3 sentence summary of the proposal.\n\
         \n\
         ## Motivation\n\
         \n\
         Why are we doing this? What problem does it solve?\n\
         \n\
         ## Goals\n\
         \n\
         What are we trying to achieve?\n\
         \n\
         ## Non-Goals\n\
         \n\
         What is explicitly out of scope?\n\
         \n\
         ## Detailed Design\n\
         \n\
         The meat of the RFC. Explain the design in enough detail that:\n\
         - Its interaction with other features is clear\n\
         - It's reasonably clear how to implement\n\
         - Corner cases are discussed\n\
         \n\
         Include code examples, diagrams, or data models where helpful.\n\
         \n\
         ## Alternatives Considered\n\
         \n\
         What other approaches were considered and why weren't they chosen?\n\
         \n\
         ## Open Questions\n\
         \n\
         What parts of the design still need to be figured out?\n\
         \n\
         ## Implementation Phases\n\
         \n\
         If this is a large change, how should it be broken down?\n"
    );
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| format!("create RFC {}", path.display()))?;
    file.write_all(template.as_bytes())
        .context("write RFC draft")?;
    Ok(path)
}

struct Hit {
    score: usize,
    rendered: String,
}

fn search(directory: &Path, status: &str, terms: &[String]) -> Result<String> {
    let mut files = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    files.sort_by_key(|entry| entry.file_name());
    let width = usize::from(console::Term::stdout().size().1.saturating_sub(4).max(20));
    let mut hits: Vec<Hit> = Vec::new();
    for entry in files {
        let filename = entry.file_name().to_string_lossy().into_owned();
        let numbered = filename.split_once('-').is_some_and(|(number, name)| {
            number.len() == 4 && number.bytes().all(|c| c.is_ascii_digit()) && name.ends_with(".md")
        });
        if !(numbered || filename == "RFC.md") || !entry.file_type()?.is_file() {
            continue;
        }
        let rfc = parse(&fs::read_to_string(entry.path())?, &filename);
        if rfc.status == "Unknown" {
            eprintln!("warning: {filename} has no parseable \"- **Status:**\" line");
        }
        if !status.eq_ignore_ascii_case("all") && !rfc.status.eq_ignore_ascii_case(status) {
            continue;
        }
        let title_matches_all = !terms.is_empty()
            && terms
                .iter()
                .all(|term| rfc.title.to_lowercase().contains(term));
        let matches: Vec<_> = rfc
            .sections
            .iter()
            .filter(|section| {
                let text = format!("{}\n{}", rfc.title, section.text).to_lowercase();
                terms.iter().all(|term| text.contains(term))
            })
            .collect();
        if matches.is_empty() {
            continue;
        }
        let score = matches.len() + if title_matches_all { 5 } else { 0 };

        let mut rendered = String::new();
        let updated_suffix = rfc
            .updated
            .as_ref()
            .map(|updated| format!("  {}", style(format!("(updated {updated})")).dim()))
            .unwrap_or_default();
        rendered.push_str(&format!(
            "{}  {}{}\n",
            style(&rfc.title).cyan().bold(),
            style(&rfc.status).dim(),
            updated_suffix
        ));
        for (label, value) in &rfc.relations {
            rendered.push_str(&format!(
                "  {} {}\n",
                style(format!("{label}:")).dim(),
                value
            ));
        }
        for section in matches
            .into_iter()
            .take(if terms.is_empty() { 1 } else { usize::MAX })
        {
            if !terms.is_empty() {
                rendered.push_str(&format!("  {}\n", style(&section.heading).bold()));
            }
            if !terms.is_empty() {
                let excerpt = section
                    .text
                    .lines()
                    .skip(1)
                    .find(|line| {
                        !line.trim().is_empty()
                            && terms.iter().any(|term| line.to_lowercase().contains(term))
                    })
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if !excerpt.is_empty() {
                    rendered.push_str(&format!("  {}\n", truncate_str(&excerpt, width, "...")));
                }
            }
            rendered.push_str(&format!(
                "  {}\n",
                style(format!(
                    "[{}:{}]({}#{})",
                    directory.join(&filename).display(),
                    section.line,
                    directory.join(&filename).display(),
                    section.anchor
                ))
                .dim()
            ));
        }
        rendered.push('\n');
        hits.push(Hit { score, rendered });
    }
    // Stable sort: ties (including the "no query" listing, where every
    // score is equal) keep the original filename order.
    hits.sort_by_key(|hit| std::cmp::Reverse(hit.score));

    let mut output = String::new();
    for hit in hits {
        output.push_str(&hit.rendered);
    }
    if output.is_empty() {
        output.push_str("No matching RFCs.\n");
    }
    Ok(output)
}

pub fn run(args: Args) -> Result<()> {
    let directory = match args.directory {
        Some(directory) => directory,
        None => match env::var_os("DEVENV_ROOT") {
            Some(root) => PathBuf::from(root).join("rfcs"),
            None => crate::repo::root()
                .context("find RFC root; use --directory PATH outside a Git checkout")?
                .join("rfcs"),
        },
    };
    if let Some(title) = args.new {
        let path = create_new_rfc(&directory, &title)?;
        println!("{}", path.display());
        return Ok(());
    }
    let terms = args
        .query
        .iter()
        .flat_map(|s| s.split_whitespace().map(str::to_lowercase))
        .collect::<Vec<_>>();
    print!("{}", search(&directory, &args.status, &terms)?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_and_skips_headings_inside_fences() {
        let rfc = parse(
            "# RFC 0001: Storage\r\n- **Status:** Accepted\r\n## Design\r\nRedis\r\n~~~~\r\n# comment\r\n~~~\r\n~~~~\r\n## Design\r\nPostgres",
            "0001-storage.md",
        );
        assert_eq!(rfc.title, "RFC 0001: Storage");
        assert_eq!(rfc.status, "Accepted");
        assert_eq!(rfc.sections.len(), 3);
        assert_eq!(rfc.sections[1].anchor, "design");
        assert_eq!(rfc.sections[1].line, 3);
        assert!(rfc.sections[1].text.contains("# comment"));
        assert_eq!(rfc.sections[2].anchor, "design-1");
    }

    #[test]
    fn preserves_text_without_headings_or_metadata() {
        let rfc = parse("Use Redis for sessions.", "0002-storage.md");
        assert_eq!(rfc.title, "0002-storage.md");
        assert_eq!(rfc.status, "Unknown");
        assert_eq!(rfc.sections[0].text, "Use Redis for sessions.");
    }

    #[test]
    fn parses_updated_and_relations() {
        let rfc = parse(
            "# RFC 0011: Sessions v2\n- **Status:** Accepted\n- **Updated:** 2026-01-02\n- **Supersedes:** 0003\n## Design\nRedis",
            "0011-sessions-v2.md",
        );
        assert_eq!(rfc.updated.as_deref(), Some("2026-01-02"));
        assert_eq!(
            rfc.relations,
            vec![("Supersedes".to_string(), "0003".to_string())]
        );
    }

    #[test]
    fn today_produces_a_plausible_iso_date() {
        let date = today().unwrap();
        let parts: Vec<&str> = date.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!((parts[0].len(), parts[1].len(), parts[2].len()), (4, 2, 2));
        assert!(parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())));
        assert!(parts[0].parse::<u32>().unwrap() >= 2024);
    }

    #[test]
    fn slugify_handles_punctuation_and_case() {
        assert_eq!(
            slugify("SAML Proxy: University Auth!"),
            "saml-proxy-university-auth"
        );
        assert_eq!(slugify("  leading and trailing  "), "leading-and-trailing");
    }
}
