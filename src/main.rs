//! Bridge Gerrit changes -> Circus CI -> Verified label.
//!
//! Polls Gerrit for open changes with unreviewed patchsets, triggers
//! Circus evaluations, and posts Verified labels back.

use std::{
   collections::HashSet,
   time::Duration,
};

use anyhow::{
   Context as _,
   Result,
   anyhow,
};
use serde::{
   Deserialize,
   Serialize,
};

pub mod gerrit;

struct Config {
   circus_url:   String,
   circus_key:   String,
   gerrit_query: String,
   poll_int:     u64,
   poll_max:     u64,
}

impl Config {
   fn from_env() -> Result<Self> {
      Ok(Self {
         circus_url:   std::env::var("CIRCUS_URL").context("CIRCUS_URL must be set")?,
         circus_key:   std::env::var("CIRCUS_API_KEY").context("CIRCUS_API_KEY must be set")?,
         gerrit_query: std::env::var("GERRIT_CHANGE_QUERY")
            .unwrap_or_else(|_| "status:open+-is:wip".into()),
         poll_int:     std::env::var("POLL_INTERVAL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30),
         poll_max:     std::env::var("POLL_TIMEOUT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(3600),
      })
   }
}

fn circus_agent() -> ureq::Agent {
   ureq::Agent::config_builder()
      .timeout_global(Some(Duration::from_secs(15)))
      .build()
      .new_agent()
}

fn circus_auth(cfg: &Config) -> String {
   format!("Bearer {}", cfg.circus_key)
}

fn circus_get<T: serde::de::DeserializeOwned>(cfg: &Config, path: &str) -> Result<T> {
   let url = format!("{}/api/v1{}", cfg.circus_url.trim_end_matches('/'), path);
   let body = circus_agent()
      .get(&url)
      .header("Authorization", &circus_auth(cfg))
      .call()
      .map_err(|e| anyhow!("Circus GET {path}: {e}"))?
      .into_body()
      .read_to_string()?;
   serde_json::from_str(&body).context("failed to parse Circus GET response")
}

fn circus_post<T: serde::de::DeserializeOwned>(
   cfg: &Config,
   path: &str,
   body: &impl serde::Serialize,
) -> Result<T> {
   let url = format!("{}/api/v1{}", cfg.circus_url.trim_end_matches('/'), path);
   let json_bytes = serde_json::to_vec(body)?;
   let resp = circus_agent()
      .post(&url)
      .header("Authorization", &circus_auth(cfg))
      .header("Content-Type", "application/json")
      .send(json_bytes.as_slice())
      .map_err(|e| anyhow!("Circus POST {path}: {e}"))?;
   let body = resp.into_body().read_to_string()?;
   serde_json::from_str(&body).context("failed to parse Circus POST response")
}

#[derive(Debug, Deserialize)]
struct Project {
   id:   String,
   name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct Jobset {
   id:           String,
   name:         String,
   branch:       Option<String>,
   trigger_mode: Option<String>,
}

#[derive(Debug, Serialize)]
struct TriggerEval {
   jobset_id:   String,
   commit_hash: String,
}

#[derive(Debug, Deserialize)]
struct Build {
   status: String,
}

fn resolve_jobset(
   cfg: &Config,
   project_map: &std::collections::HashMap<String, String>,
   gerrit_project: &str,
   branch: &str,
) -> Result<Jobset> {
   let circus_name = project_map
      .get(gerrit_project)
      .cloned()
      .unwrap_or_else(|| gerrit_project.to_owned());

   let raw: serde_json::Value = circus_get(cfg, "/projects").context("failed to list projects")?;
   let projects: Vec<Project> = if let Some(items) = raw.get("items") {
      serde_json::from_value(items.clone())?
   } else {
      serde_json::from_value(raw)?
   };

   let proj = projects
      .iter()
      .find(|p| p.name == circus_name)
      .ok_or_else(|| anyhow!("project '{circus_name}' not found in Circus"))?;

   let raw_js: serde_json::Value = circus_get(cfg, &format!("/projects/{}/jobsets", proj.id))
      .context("failed to list jobsets")?;
   let jobsets: Vec<Jobset> = if let Some(items) = raw_js.get("items") {
      serde_json::from_value(items.clone())?
   } else {
      serde_json::from_value(raw_js)?
   };

   let js = jobsets
      .iter()
      .find(|j| {
         (j.branch.as_deref() == Some(branch) || j.branch.is_none())
            && j.trigger_mode.as_deref() == Some("source_change")
      })
      .ok_or_else(|| {
         let names: Vec<_> = jobsets.iter().map(|j| j.name.clone()).collect();
         anyhow!(
            "no source_change jobset for branch '{branch}' in project '{circus_name}'. available: \
             {names:?}"
         )
      })?;

   Ok((*js).clone())
}

fn trigger_evaluation(cfg: &Config, jobset_id: &str, commit: &str) -> Result<String> {
   let body = TriggerEval {
      jobset_id:   jobset_id.to_owned(),
      commit_hash: commit.to_owned(),
   };
   let resp: serde_json::Value =
      circus_post(cfg, "/evaluations/trigger", &body).context("failed to trigger evaluation")?;
   let eval_id = resp
      .get("id")
      .and_then(|v| v.as_str())
      .ok_or_else(|| anyhow!("trigger response missing 'id': {resp}"))?;
   eprintln!(
      "  evaluation {} triggered for commit {}",
      &eval_id[..8],
      &commit[..8]
   );
   Ok(eval_id.to_owned())
}

fn poll_builds(cfg: &Config, eval_id: &str, deadline: std::time::Instant) -> Result<String> {
   eprintln!("  polling evaluation {}...", &eval_id[..8]);
   let interval = Duration::from_secs(cfg.poll_int);

   loop {
      if std::time::Instant::now() > deadline {
         return Ok("timeout".into());
      }

      let raw: serde_json::Value = circus_get(cfg, &format!("/builds?evaluation_id={eval_id}"))
         .context("failed to list builds")?;
      let builds: Vec<Build> = if let Some(items) = raw.get("items") {
         serde_json::from_value(items.clone())?
      } else {
         serde_json::from_value(raw)?
      };

      let running = builds
         .iter()
         .filter(|b| b.status == "running" || b.status == "pending")
         .count();
      let failed = builds
         .iter()
         .filter(|b| {
            matches!(
               b.status.as_str(),
               "failed" | "aborted" | "cancelled" | "unsupported_system"
            )
         })
         .count();
      let ok = builds.iter().filter(|b| b.status == "succeeded").count();

      if running == 0 {
         eprintln!("  builds done: {ok} ok, {failed} fail");
         return if failed > 0 {
            Ok("failed".into())
         } else {
            Ok("succeeded".into())
         };
      }

      eprintln!("  {ok} ok, {failed} fail, {running} running...");
      std::thread::sleep(interval);
   }
}

/// A Gerrit change entry (from the list endpoint).
#[derive(Debug, Deserialize)]
struct GerritChange {
   #[serde(rename = "_change_number")]
   _change_number:   Option<i64>,
   id:               String,
   project:          String,
   branch:           String,
   #[serde(default)]
   current_revision: Option<String>,
   #[serde(default)]
   revisions:        Option<std::collections::BTreeMap<String, GerritRevision>>,
}

#[derive(Debug, Deserialize)]
struct GerritRevision {
   #[serde(default)]
   _number: i64,
}

/// Track which (change, patchset) pairs we've already processed.
struct Processed {
   seen: HashSet<(String, i64)>,
}

impl Processed {
   fn new() -> Self {
      Self {
         seen: HashSet::new(),
      }
   }

   fn is_new_patchset(&mut self, change_id: &str, patchset: i64) -> bool {
      self.seen.insert((change_id.to_owned(), patchset))
   }
}

fn load_project_map() -> std::collections::HashMap<String, String> {
   // Edit this to map Gerrit project names to Circus project names
   // when they differ (TODO: should be a config option):
   //   map.insert("gerrit/project".into(), "circus-project".into());
   std::collections::HashMap::new()
}

fn process_change(
   cfg: &Config,
   gerrit_cfg: &gerrit::Config,
   project_map: &std::collections::HashMap<String, String>,
   change: &GerritChange,
) {
   let Some(rev) = &change.current_revision else {
      eprintln!("  [{}] no current revision, skipping", &change.id[..8]);
      return;
   };

   let patchset_num = change
      .revisions
      .as_ref()
      .and_then(|revs| revs.get(rev))
      .map(|r| r._number)
      .unwrap_or(0);

   eprintln!();
   eprintln!("=== change ===");
   eprintln!("  project:  {}", change.project);
   eprintln!("  branch:   {}", change.branch);
   eprintln!("  change:   {}", &change.id[..8]);
   eprintln!("  patchset: {}", patchset_num);
   eprintln!("  commit:   {}", &rev[..8]);

   // Resolve jobset.
   let js = match resolve_jobset(cfg, project_map, &change.project, &change.branch) {
      Ok(j) => j,
      Err(e) => {
         eprintln!("  ERROR: {e}");
         return;
      },
   };

   // Post "running" label.
   if let Err(e) = gerrit::set_verified(
      gerrit_cfg,
      &change.id,
      rev,
      0,
      &format!("CI started — patchset {patchset_num}"),
   ) {
      eprintln!("  WARNING: label post failed: {e:#}");
   }

   // Trigger eval.
   let eval_id = match trigger_evaluation(cfg, &js.id, rev) {
      Ok(id) => id,
      Err(e) => {
         eprintln!("  ERROR: {e:#}");
         return;
      },
   };

   // Poll for result.
   let deadline = std::time::Instant::now() + Duration::from_secs(cfg.poll_max);
   let outcome = match poll_builds(cfg, &eval_id, deadline) {
      Ok(o) => o,
      Err(e) => {
         eprintln!("  ERROR: {e}");
         "error".to_owned()
      },
   };

   // Post final label.
   let verified = match outcome.as_str() {
      "succeeded" => 1,
      "timeout" | "error" => 0,
      _ => -1,
   };
   if let Err(e) = gerrit::set_verified(
      gerrit_cfg,
      &change.id,
      rev,
      verified,
      &format!("Circus CI finished: {outcome}"),
   ) {
      eprintln!("  WARNING: label post failed: {e}");
   }
}

/// Fetch open changes from Gerrit, picking ones we haven't processed yet.
fn fetch_pending_changes(gerrit_cfg: &gerrit::Config, query: &str) -> Result<Vec<GerritChange>> {
   let endpoint = format!("/changes/?q={}&o=CURRENT_REVISION&o=CURRENT_COMMIT", query);
   gerrit::get(gerrit_cfg, &endpoint)
}

fn run(cfg: Config, gerrit_cfg: gerrit::Config) -> Result<()> {
   let project_map = load_project_map();
   let mut processed = Processed::new();
   let interval = Duration::from_secs(cfg.poll_int);

   loop {
      let changes = match fetch_pending_changes(&gerrit_cfg, &cfg.gerrit_query) {
         Ok(c) => c,
         Err(e) => {
            eprintln!("ERROR: failed to fetch changes: {e}, retrying...");
            std::thread::sleep(interval);
            continue;
         },
      };

      eprintln!("polled {} open changes", changes.len());

      for change in &changes {
         let patch_num = change
            .revisions
            .as_ref()
            .and_then(|revs| {
               change
                  .current_revision
                  .as_ref()
                  .and_then(|rev| revs.get(rev))
            })
            .map(|r| r._number)
            .unwrap_or(0);

         if !processed.is_new_patchset(&change.id, patch_num) {
            continue;
         }

         eprintln!(
            "new patchset {} for change {}, processing...",
            patch_num,
            &change.id[..8]
         );
         process_change(&cfg, &gerrit_cfg, &project_map, change);
      }

      std::thread::sleep(interval);
   }
}

fn main() -> Result<()> {
   let cfg = Config::from_env()?;
   let gerrit_cfg = gerrit::Config::from_env()?;
   run(cfg, gerrit_cfg)
}
