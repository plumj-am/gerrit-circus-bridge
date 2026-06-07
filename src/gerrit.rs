//! Gerrit REST API helpers for the bridge.

use std::{
   env,
   time::Duration,
};

use anyhow::{
   Context as _,
   Result,
   anyhow,
};
use base64::Engine as _;
use serde::{
   Deserialize,
   Serialize,
};
use ureq::Agent;

/// Gerrit connection parameters, read from environment.
pub struct Config {
   pub gerrit_url: String,
   pub username:   String,
   pub password:   String,
   pub agent:      Agent,
}

impl Config {
   pub fn from_env() -> Result<Self> {
      Ok(Self {
         gerrit_url: env::var("GERRIT_URL")
            .context("Gerrit base URL (no trailing slash) must be set in GERRIT_URL")?,
         username:   env::var("GERRIT_USERNAME")
            .context("Gerrit username must be set in GERRIT_USERNAME")?,
         password:   env::var("GERRIT_PASSWORD")
            .context("Gerrit password must be set in GERRIT_PASSWORD")?,
         agent:      Agent::config_builder()
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent("gerrit-circus-bridge")
            .build()
            .new_agent(),
      })
   }
}

#[derive(Debug, Deserialize)]
pub struct ChangeInfo {
   pub id: String,
}

/// Gerrit POST /review endpoint structure.
#[derive(Debug, Serialize)]
pub struct ReviewInput {
   pub labels:  serde_json::Value,
   pub message: String,
}

const GERRIT_RESPONSE_PREFIX: &str = ")]}'";

fn auth_header(username: &str, password: &str) -> String {
   let creds = base64::engine::general_purpose::STANDARD.encode(format!("{username}:{password}"));
   format!("Basic {creds}")
}

fn base_url(cfg: &Config) -> String {
   cfg.gerrit_url.trim_end_matches('/').to_owned()
}

/// GET a Gerrit endpoint and deserialise the JSON body (skipping `)]}'`).
pub fn get<T: serde::de::DeserializeOwned>(cfg: &Config, endpoint: &str) -> Result<T> {
   let url = format!("{}/a{}", base_url(cfg), endpoint);
   let body = cfg
      .agent
      .get(&url)
      .header("Authorization", &auth_header(&cfg.username, &cfg.password))
      .call()
      .map_err(|e| gerrit_err(e, "GET"))?
      .into_body()
      .read_to_string()?;

   let stripped = body.strip_prefix(GERRIT_RESPONSE_PREFIX).unwrap_or(&body);
   serde_json::from_str(stripped).context("failed to parse Gerrit GET response")
}

/// POST a JSON body to a Gerrit endpoint.
pub fn post<T: serde::de::DeserializeOwned>(
   cfg: &Config,
   endpoint: &str,
   body: &impl serde::Serialize,
) -> Result<T> {
   let url = format!("{}/a{}", base_url(cfg), endpoint);
   let json_bytes = serde_json::to_vec(body).context("failed to serialise POST body")?;

   let resp = cfg
      .agent
      .post(&url)
      .header("Authorization", &auth_header(&cfg.username, &cfg.password))
      .send(json_bytes.as_slice())
      .map_err(|e| gerrit_err(e, "POST"))?;

   let body = resp.into_body().read_to_string()?;
   let stripped = body.strip_prefix(GERRIT_RESPONSE_PREFIX).unwrap_or(&body);
   serde_json::from_str(stripped).context("failed to parse Gerrit POST response")
}

/// Set the `Verified` label on a change/revision.
///
/// `verified` — `1` (pass), `-1` (fail), `0` (neutral / running).
pub fn set_verified(
   cfg: &Config,
   change_id: &str,
   revision: &str,
   verified: i32,
   message: &str,
) -> Result<()> {
   let endpoint = format!("/changes/{change_id}/revisions/{revision}/review");
   let input = ReviewInput {
      labels:  serde_json::json!({ "Verified": verified }),
      message: message.to_owned(),
   };
   let _: serde_json::Value = post(cfg, &endpoint, &input)?;
   Ok(())
}

fn gerrit_err(e: ureq::Error, method: &str) -> anyhow::Error {
   match e {
      ureq::Error::StatusCode(code) => {
         anyhow!("Gerrit {method} returned status {code}")
      },
      e => anyhow!("Gerrit {method} failed: {e}"),
   }
}
