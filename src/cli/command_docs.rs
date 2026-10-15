use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

use clap::{ColorChoice, Command, CommandFactory};

use super::Cli;

fn help_text(command: &mut Command) -> String {
    command
        .render_long_help()
        .to_string()
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render(command: &Command, page: &mut String, depth: usize) {
    let name = command.get_bin_name().unwrap_or(command.get_name());
    writeln!(page, "{} `{name}`\n", "#".repeat(depth)).unwrap();
    if depth == 1 {
        page.push_str(
            "Global options apply to every command below; see the [command index](index.md).\n\n",
        );
    }
    let mut help = command.clone().color(ColorChoice::Never).term_width(88);
    let args: Vec<_> = help
        .get_arguments()
        .map(|arg| (arg.get_id().clone(), arg.is_global_set()))
        .collect();
    for (id, global) in args {
        help = help.mut_arg(id, |arg| arg.hide_env_values(true).hide(global));
    }
    writeln!(page, "```text\n{}\n```\n", help_text(&mut help)).unwrap();
    for child in command.get_subcommands() {
        if child.get_name() != "help" && !child.is_hide_set() {
            render(child, page, depth + 1);
        }
    }
}

#[test]
fn command_reference_is_current() {
    let mut command = Cli::command().bin_name("collar");
    command.build();
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/reference/commands");
    let mut pages = BTreeMap::new();
    let mut index = String::from(
        "# Command reference\n\nThese pages are generated from Collar's Clap definitions. Use the help shipped\nwith your installed version when it differs from this site. Global options are\nlisted here and apply to every subcommand.\n\n",
    );
    for child in command.get_subcommands() {
        if child.get_name() == "help" || child.is_hide_set() {
            continue;
        }
        let name = child.get_name();
        writeln!(index, "- [`collar {name}`]({name}.md)").unwrap();
        let mut page = String::new();
        render(child, &mut page, 1);
        pages.insert(format!("{name}.md"), format!("{}\n", page.trim_end()));
    }
    index.push_str("\n## Global options\n\n```text\n");
    let mut root = command.clone().color(ColorChoice::Never).term_width(88);
    let ids: Vec<_> = root
        .get_arguments()
        .map(|arg| arg.get_id().clone())
        .collect();
    for id in ids {
        root = root.mut_arg(id, |arg| arg.hide_env_values(true));
    }
    writeln!(index, "{}\n```", help_text(&mut root)).unwrap();
    pages.insert("index.md".into(), index);

    if std::env::var_os("COLLAR_UPDATE_DOCS").is_some() {
        std::fs::create_dir_all(&directory).unwrap();
        for entry in std::fs::read_dir(&directory).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| extension == "md")
                && !pages.contains_key(path.file_name().unwrap().to_str().unwrap())
            {
                std::fs::remove_file(path).unwrap();
            }
        }
        for (name, content) in &pages {
            std::fs::write(directory.join(name), content).unwrap();
        }
    }
    for (name, expected) in pages {
        let path = directory.join(&name);
        let actual = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            actual == expected,
            "{} is outdated; run COLLAR_UPDATE_DOCS=1 cargo test command_reference_is_current",
            path.display()
        );
    }
}
