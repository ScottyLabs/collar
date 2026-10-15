use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::Args;
use reqwest::Method;
use serde_json::{Map, Value};

use super::input::read_file;
use crate::{auth::api, client::host_url, config::Config};

#[derive(Args)]
pub(super) struct ApiArgs {
    pub(super) endpoint: String,
    #[arg(short = 'X', long)]
    pub(super) method: Option<String>,
    /// Add a string field; fields imply POST unless --method is given.
    #[arg(short = 'f', long = "raw-field")]
    pub(super) raw_fields: Vec<String>,
    /// Add a typed JSON field; @file and @- read a string from a file or stdin.
    #[arg(short = 'F', long = "field")]
    pub(super) fields: Vec<String>,
    /// Read a JSON request body from a file, or - for stdin.
    #[arg(long, conflicts_with_all = ["raw_fields", "fields"])]
    pub(super) input: Option<PathBuf>,
}

pub(super) fn run(args: ApiArgs, host: Option<&str>, config: &Config) -> Result<()> {
    let api = api(host_url(host.unwrap_or(config.host()))?, config)?;
    ensure!(
        !args.endpoint.contains([':', '?', '#', '%']),
        "API endpoint must be a relative path without a query or URL escapes; use -f/-F for fields"
    );
    let path: Vec<_> = args.endpoint.trim_start_matches('/').split('/').collect();
    let mut fields = Map::new();
    for (typed, entries) in [(false, args.raw_fields), (true, args.fields)] {
        for field in entries {
            let (name, value) = field
                .split_once('=')
                .context("API fields must be KEY=VALUE")?;
            ensure!(
                !name.is_empty() && !name.contains(['[', ']']),
                "nested or empty API field names are not supported"
            );
            let value = if typed {
                if let Some(path) = value.strip_prefix('@') {
                    Value::String(read_file(Path::new(path))?)
                } else {
                    serde_json::from_str(value).unwrap_or_else(|_| Value::String(value.into()))
                }
            } else {
                Value::String(value.into())
            };
            ensure!(
                fields.insert(name.into(), value).is_none(),
                "duplicate API field: {name}"
            );
        }
    }
    let method = match args.method {
        Some(method) => {
            Method::from_bytes(method.to_uppercase().as_bytes()).context("invalid HTTP method")?
        }
        None if !fields.is_empty() || args.input.is_some() => Method::POST,
        None => Method::GET,
    };
    let mut query = Vec::new();
    let body = if matches!(method, Method::GET | Method::HEAD) {
        ensure!(args.input.is_none(), "GET/HEAD cannot use --input");
        for (key, value) in fields {
            query.push((
                key,
                match value {
                    Value::String(text) => text,
                    value => value.to_string(),
                },
            ));
        }
        None
    } else if let Some(path) = args.input {
        Some(serde_json::from_str(&read_file(&path)?).context("parse API request JSON")?)
    } else if fields.is_empty() {
        None
    } else {
        Some(Value::Object(fields))
    };
    let text = api.text(method, &path, &query, body.as_ref())?;
    if !text.is_empty() {
        let value: Value = serde_json::from_str(&text).context("API response is not JSON")?;
        println!("{}", serde_json::to_string_pretty(&value)?);
    }
    Ok(())
}
