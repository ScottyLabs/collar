use anyhow::{Context, Result};
use url::Url;

use crate::client::{Api, host_url};
use crate::config::Config;

pub fn api(origin: Url, config: &Config) -> Result<Api> {
    let configured = match std::env::var("COLLAR_HOST") {
        Ok(host) => host,
        Err(std::env::VarError::NotPresent) => config.host().into(),
        Err(error) => return Err(error).context("read COLLAR_HOST"),
    };
    let token = if origin == host_url(&configured)? {
        match std::env::var("COLLAR_TOKEN") {
            Ok(token) => Some(token),
            Err(std::env::VarError::NotPresent) => None,
            Err(error) => return Err(error).context("read COLLAR_TOKEN"),
        }
    } else {
        None
    };
    Api::new(origin, token)
}
