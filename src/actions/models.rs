use std::collections::BTreeMap;
use std::fmt;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub const RUN_FIELDS: &[&str] = &[
    "id",
    "index_in_repo",
    "workflow_id",
    "status",
    "event",
    "title",
    "commit_sha",
    "prettyref",
    "created",
    "started",
    "stopped",
    "html_url",
    "trigger_event",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Unknown,
    Waiting,
    Running,
    Success,
    Failure,
    Cancelled,
    Skipped,
    Blocked,
    Other(String),
}

impl Status {
    pub fn parse(value: &str) -> Self {
        match value {
            "unknown" => Self::Unknown,
            "waiting" => Self::Waiting,
            "running" => Self::Running,
            "success" => Self::Success,
            "failure" => Self::Failure,
            "cancelled" => Self::Cancelled,
            "skipped" => Self::Skipped,
            "blocked" => Self::Blocked,
            value => Self::Other(value.into()),
        }
    }

    pub fn is_known(&self) -> bool {
        !matches!(self, Self::Other(_))
    }

    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Success | Self::Failure | Self::Cancelled | Self::Skipped
        )
    }
}

impl fmt::Display for Status {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Unknown => "unknown",
            Self::Waiting => "waiting",
            Self::Running => "running",
            Self::Success => "success",
            Self::Failure => "failure",
            Self::Cancelled => "cancelled",
            Self::Skipped => "skipped",
            Self::Blocked => "blocked",
            Self::Other(value) => value,
        })
    }
}

impl Serialize for Status {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Status {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        String::deserialize(deserializer).map(|value| Self::parse(&value))
    }
}

#[derive(Deserialize)]
pub struct RunList {
    total_count: Option<u64>,
    workflow_runs: Option<Vec<RunRecord>>,
}

impl RunList {
    pub fn into_records(self) -> Result<Vec<RunRecord>> {
        self.total_count
            .context("Forgejo action run response has no numeric total_count")?;
        self.workflow_runs
            .context("Forgejo action run response has no workflow_runs array")
    }
}

// Nullable wire fields retain the command's precise missing-field diagnostics.
#[derive(Deserialize)]
pub struct RunRecord {
    id: Option<u64>,
    status: Option<Status>,
    commit_sha: Option<String>,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

#[derive(Serialize)]
pub struct Run {
    pub id: u64,
    pub status: Status,
    pub commit_sha: String,
    // Preserve additional provider metadata, including nulls, in view/watch JSON.
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

impl TryFrom<RunRecord> for Run {
    type Error = anyhow::Error;

    fn try_from(record: RunRecord) -> Result<Self> {
        Ok(Self {
            id: record.id.context("Forgejo action run has no numeric id")?,
            status: record.status.context("Forgejo action run has no status")?,
            commit_sha: record
                .commit_sha
                .context("Forgejo action run has no commit_sha")?,
            metadata: record.metadata,
        })
    }
}

impl Run {
    pub fn summary(&self) -> Result<Value> {
        let mut value = serde_json::to_value(self)?;
        value
            .as_object_mut()
            .context("serialized action run must be an object")?
            .retain(|name, _| RUN_FIELDS.contains(&name.as_str()));
        Ok(value)
    }

    pub fn detail(&self, name: &str) -> String {
        self.metadata.get(name).map(display).unwrap_or_default()
    }
}

#[derive(Deserialize, Serialize)]
pub struct Job {
    pub id: u64,
    pub name: String,
    pub status: Status,
    // The API may omit attempt or return null; keep those distinct in jobs JSON.
    #[serde(
        default,
        deserialize_with = "present",
        skip_serializing_if = "Option::is_none"
    )]
    pub attempt: Option<Option<u64>>,
    #[serde(flatten)]
    metadata: BTreeMap<String, Value>,
}

impl Job {
    pub fn detail(&self, name: &str) -> String {
        self.metadata.get(name).map(display).unwrap_or_default()
    }
}

fn present<'de, D, T>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

#[derive(Serialize)]
pub struct JobLog {
    pub id: u64,
    pub name: String,
    pub status: Status,
    pub attempt: Option<u64>,
    pub log: String,
}

pub fn display(value: &Value) -> String {
    value
        .as_str()
        .map(str::to_owned)
        .unwrap_or_else(|| match value {
            Value::Null => String::new(),
            _ => value.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn run_summary_retains_null_metadata_but_view_retains_extra_fields() {
        let value = json!({
            "id": 1,
            "status": "waiting",
            "commit_sha": "head",
            "created": null,
            "custom": {"enabled": true},
        });
        let record: RunRecord = serde_json::from_value(value.clone()).unwrap();
        let run = Run::try_from(record).unwrap();
        assert_eq!(serde_json::to_value(&run).unwrap(), value);
        let summary = run.summary().unwrap();
        assert!(summary.get("custom").is_none());
        assert_eq!(summary.get("created"), Some(&Value::Null));
    }

    #[test]
    fn preserves_job_attempt_presence_and_provider_metadata() {
        for attempt in [None, Some(Value::Null), Some(json!(2))] {
            let mut value = json!({
                "id": 1,
                "name": "test",
                "status": "success",
                "custom": null,
            });
            if let Some(attempt) = attempt {
                value["attempt"] = attempt;
            }
            let job: Job = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(serde_json::to_value(job).unwrap(), value);
        }
    }

    #[test]
    fn preserves_future_statuses_for_reads_without_treating_them_as_complete() {
        let status: Status = serde_json::from_value(json!("future-status")).unwrap();
        assert!(!status.is_known());
        assert!(!status.is_terminal());
        assert_eq!(
            serde_json::to_value(status).unwrap(),
            json!("future-status")
        );
    }

    #[test]
    fn rejects_invalid_job_ids_and_attempts() {
        for value in [
            json!({
                "id": "wrong",
                "name": "test",
                "status": "running",
            }),
            json!({
                "id": 1,
                "name": "test",
                "status": "running",
                "attempt": -1,
            }),
        ] {
            assert!(serde_json::from_value::<Job>(value).is_err());
        }
    }
}
