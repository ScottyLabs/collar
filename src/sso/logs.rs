use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use super::{Endpoints, Session, client, now, read_json};

#[derive(Serialize, Deserialize)]
pub struct Line {
    pub timestamp: String,
    pub line: String,
}

pub fn read(
    endpoints: &Endpoints,
    session: &Session,
    project: &str,
    branch: &str,
    commit: &str,
    limit: u64,
    minutes: u64,
) -> Result<Vec<Line>> {
    ensure!(
        endpoints.matches(session),
        "saved SSO session belongs to different endpoints"
    );
    ensure!(
        (1..=1000).contains(&limit) && (1..=43200).contains(&minutes),
        "deployment log query exceeds allowed bounds"
    );
    ensure!(
        [40, 64].contains(&commit.len()) && commit.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "deployment log query requires a full commit hash"
    );
    let query = format!(
        "{{project={}}} | branch={} | commit={}",
        serde_json::to_string(project)?,
        serde_json::to_string(branch)?,
        serde_json::to_string(commit)?
    );
    let end = now()?;
    let start = end
        .checked_sub(minutes * 60)
        .context("invalid deployment log time range")?;
    let mut url = endpoints
        .grafana
        .join("api/datasources/proxy/uid/loki/loki/api/v1/query_range")?;
    url.query_pairs_mut().extend_pairs([
        ("query", query),
        ("start", format!("{start}000000000")),
        ("end", format!("{end}000000000")),
        ("limit", limit.to_string()),
        ("direction", "backward".into()),
    ]);
    let response = client()?
        .get(url)
        .header("X-JWT-Assertion", &session.access_token)
        .send()
        .map_err(|_| anyhow::anyhow!("Grafana log query unavailable"))?;
    ensure!(
        response.status().is_success(),
        "Grafana rejected the log query (HTTP {})",
        response.status().as_u16()
    );
    #[derive(Deserialize)]
    struct Response {
        status: String,
        data: Data,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Data {
        result_type: String,
        result: Vec<Stream>,
    }
    #[derive(Deserialize)]
    struct Stream {
        values: Vec<Vec<serde_json::Value>>,
    }
    let response: Response = read_json(response)?;
    ensure!(
        response.status == "success" && response.data.result_type == "streams",
        "Grafana returned an invalid log response"
    );
    let mut lines = Vec::new();
    for stream in response.data.result {
        for value in stream.values {
            ensure!(
                lines.len() < limit as usize,
                "Grafana exceeded the log line limit"
            );
            ensure!(value.len() == 2 || value.len() == 3, "invalid log entry");
            let timestamp = value[0].as_str().context("invalid log timestamp")?;
            let timestamp_value: u128 = timestamp.parse().context("invalid log timestamp")?;
            ensure!(
                timestamp_value >= u128::from(start) * 1_000_000_000
                    && timestamp_value <= u128::from(end) * 1_000_000_000,
                "Grafana returned logs outside the requested range"
            );
            lines.push(Line {
                timestamp: timestamp.into(),
                line: value[1].as_str().context("invalid log line")?.to_owned(),
            });
        }
    }
    lines.sort_by(|left, right| {
        left.timestamp
            .len()
            .cmp(&right.timestamp.len())
            .then_with(|| left.timestamp.cmp(&right.timestamp))
    });
    Ok(lines)
}
