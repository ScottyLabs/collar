use anyhow::{Context, Result, ensure};
use clap::Args;
use serde::Serialize;
use serde_json::{Map, Value};

#[derive(Args, Debug)]
pub struct Output {
    /// Output selected JSON fields (comma-separated); omit fields for all.
    #[arg(long, value_name = "FIELDS", num_args = 0..=1, default_missing_value = "*")]
    pub json: Option<String>,
}

impl Output {
    pub fn validate_fields(&self, available: &[&str]) -> Result<()> {
        if let Some(fields) = &self.json
            && fields != "*"
        {
            for field in fields.split(',') {
                ensure!(!field.is_empty(), "JSON field names must be nonempty");
                ensure!(
                    available.contains(&field),
                    "unsupported JSON field: {field}; available fields: {}",
                    available.join(",")
                );
            }
        }
        Ok(())
    }

    pub fn print(&self, value: &impl Serialize, human: impl FnOnce()) -> Result<()> {
        if let Some(fields) = &self.json {
            let value = project(serde_json::to_value(value)?, fields)?;
            println!("{}", serde_json::to_string_pretty(&value)?);
        } else {
            human();
        }
        Ok(())
    }
}

fn project(value: Value, fields: &str) -> Result<Value> {
    if fields == "*" {
        return Ok(value);
    }
    let fields: Vec<_> = fields.split(',').collect();
    ensure!(
        fields.iter().all(|field| !field.is_empty()),
        "JSON field names must be nonempty"
    );
    fn object(value: Value, fields: &[&str]) -> Result<Value> {
        let source = value
            .as_object()
            .context("JSON projection requires objects")?;
        let mut result = Map::new();
        for field in fields {
            let value = source
                .get(*field)
                .with_context(|| format!("unsupported JSON field: {field}"))?;
            result.insert((*field).into(), value.clone());
        }
        Ok(Value::Object(result))
    }
    match value {
        Value::Array(items) => items
            .into_iter()
            .map(|item| object(item, &fields))
            .collect(),
        value => object(value, &fields),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn projects_arrays_and_rejects_unknown_fields() {
        assert_eq!(
            project(json!([{ "number": 1, "title": "hello" }]), "number").unwrap(),
            json!([{ "number": 1 }])
        );
        assert!(project(json!({ "number": 1 }), "titel").is_err());
        assert!(project(json!({ "number": 1 }), "").is_err());
    }
}
