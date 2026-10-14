use console::{Style, measure_text_width};
use std::io::IsTerminal;

const MAX_COLUMN_WIDTH: usize = 36;

#[derive(Clone, Copy)]
pub enum Status {
    Success,
    Warning,
    Error,
}

pub fn heading(title: &str) {
    println!("{}", style(Style::new().bold(), clean(title)));
}

pub fn markdown(body: &str) {
    let body = body
        .strip_prefix("---\n")
        .and_then(|rest| rest.split_once("\n---\n").map(|(_, body)| body))
        .unwrap_or(body);
    let mut code = false;
    for line in body.lines() {
        let line = clean(line);
        if line.starts_with("```") || line.starts_with("~~~") {
            code = !code;
            println!("{}", style(Style::new().dim(), line));
        } else if !code && line.starts_with('#') {
            heading(&line);
        } else {
            println!("{line}");
        }
    }
}

pub fn detail(label: &str, value: &str) {
    println!(
        "{}  {}",
        style(Style::new().dim(), clean(label)),
        clean(value)
    );
}

pub fn status(label: &str, value: &str, state: Status) {
    let color = match state {
        Status::Success => Style::new().green(),
        Status::Warning => Style::new().yellow(),
        Status::Error => Style::new().red(),
    };
    println!("{}  {}", clean(label), style(color, clean(value)));
}

pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    if headers.is_empty() {
        return;
    }
    let columns = headers.len();
    let clean_headers: Vec<_> = headers.iter().map(|value| clean_cell(value)).collect();
    let clean_rows: Vec<Vec<_>> = rows
        .iter()
        .map(|row| {
            (0..columns)
                .map(|column| clean_cell(row.get(column).map_or("", String::as_str)))
                .collect()
        })
        .collect();
    let mut widths: Vec<_> = (0..columns)
        .map(|column| {
            std::iter::once(&clean_headers[column])
                .chain(clean_rows.iter().map(|row| &row[column]))
                .map(|cell| measure_text_width(cell))
                .max()
                .unwrap_or(0)
                .clamp(1, MAX_COLUMN_WIDTH)
        })
        .collect();
    let terminal_width = if std::io::stdout().is_terminal() {
        console::Term::stdout().size().1 as usize
    } else {
        100
    };
    while widths.iter().sum::<usize>() + 2 * (columns - 1) > terminal_width {
        let Some((index, _)) = widths.iter().enumerate().max_by_key(|(_, width)| *width) else {
            break;
        };
        if widths[index] <= 4 {
            break;
        }
        widths[index] -= 1;
    }

    let headers: Vec<_> = clean_headers
        .iter()
        .zip(&widths)
        .map(|(cell, width)| truncate(cell, *width))
        .collect();
    print_row(&headers, &widths, true);
    println!(
        "{}",
        style(
            Style::new().dim(),
            widths
                .iter()
                .map(|width| "-".repeat(*width))
                .collect::<Vec<_>>()
                .join("  ")
        )
    );
    for row in &clean_rows {
        let cells: Vec<_> = row
            .iter()
            .zip(&widths)
            .map(|(cell, width)| truncate(cell, *width))
            .collect();
        print_row(&cells, &widths, false);
    }
}

fn print_row(cells: &[String], widths: &[usize], bold: bool) {
    let row = cells
        .iter()
        .zip(widths)
        .map(|(cell, width)| {
            format!(
                "{cell}{}",
                " ".repeat(width.saturating_sub(measure_text_width(cell)))
            )
        })
        .collect::<Vec<_>>()
        .join("  ");
    println!(
        "{}",
        if bold {
            style(Style::new().bold(), row)
        } else {
            row
        }
    );
}

fn style(style: Style, text: String) -> String {
    if std::io::stdout().is_terminal() && console::colors_enabled() {
        style.apply_to(text).to_string()
    } else {
        text
    }
}

fn clean(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_control() || *character == '\n')
        .collect()
}

fn clean_cell(text: &str) -> String {
    clean(text).replace('\n', " ")
}

fn truncate(text: &str, width: usize) -> String {
    if measure_text_width(text) <= width {
        return text.to_owned();
    }
    if width <= 3 {
        return ".".repeat(width);
    }
    let target = width - 3;
    let mut result = String::new();
    for character in text.chars() {
        let mut next = result.clone();
        next.push(character);
        if measure_text_width(&next) > target {
            break;
        }
        result.push(character);
    }
    result.push_str("...");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncation_respects_visible_width_and_utf8() {
        let result = truncate("界abcdef", 6);
        assert_eq!(result, "界a...");
        assert_eq!(measure_text_width(&result), 6);
    }

    #[test]
    fn controls_are_removed_and_cells_are_single_line() {
        assert_eq!(clean("a\u{1b}[31m\tb\nc"), "a[31mb\nc");
        assert_eq!(clean_cell("a\nb"), "a b");
    }

    #[test]
    fn empty_rows_still_allow_a_header() {
        table(&["Header"], &[]);
    }
}
