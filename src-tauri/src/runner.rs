//! Runs a project's queue with a pool of workers. One supervisor task per running project.
//!
//! - Pause: workers finish the image they are drawing (it is already paid for), then wait.
//! - Stop: the same, then the run ends; queued images stay queued for the next run.
//! - Budget: checked before every request; the run stops at the cap.
//! - Auth and quota errors stop the run at once and put the image back in the queue
//!   unmarked, so the next run resumes exactly there. Nothing polls a blocked account.

use crate::error::AppResult;
use crate::gates::{self, GateMode, Verdict};
use crate::openai::{ApiError, ImageRequest, OpenAi, DEFAULT_BASE};
use crate::pricing::{self, PriceTable};
use crate::store::{full_prompt, new_id, Item, Project, Store};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::watch;

pub const MAX_WORKERS: usize = 8;

pub const STYLE_LEAD: &str = "The attached image(s) are style references only. Draw a completely new image of the \
subject described below in exactly the same art style as the references: the same rendering, line weight, shading, \
level of detail, palette treatment, framing and background. Do not copy the reference subjects; take every feature \
of the new image from the description.";

pub const REDO_LEAD: &str = "Edit the attached image. Apply only the change below and keep everything else the same: \
the subject's identity, the art style, the framing and the background.";

pub const BG_FIX: &str = "Make the background fully transparent. Keep the subject exactly as it is, fully opaque, \
with no see-through patches.";

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "state", content = "reason", rename_all = "lowercase")]
pub enum RunState {
    Idle,
    Running,
    Paused,
    Stopping,
    Stopped(String),
}

/// Where the runner reports progress. The app forwards these as UI events.
pub trait Notify: Send + Sync {
    fn item(&self, item: &Item);
    fn run(&self, project_id: &str, state: &RunState);
    fn spend(&self, project_id: &str, spent_usd: f64);
}

pub type KeySource = Arc<dyn Fn() -> AppResult<Option<String>> + Send + Sync>;

struct RunCtl {
    state: watch::Sender<RunState>,
    concurrency: AtomicUsize,
}

impl RunCtl {
    fn get(&self) -> RunState {
        self.state.borrow().clone()
    }
    fn stop(&self, reason: impl Into<String>) {
        let reason = reason.into();
        self.state.send_modify(|s| {
            if !matches!(s, RunState::Stopped(_)) {
                *s = RunState::Stopped(reason)
            }
        });
    }
}

pub struct Runner {
    store: Arc<Store>,
    notify: Arc<dyn Notify>,
    key: KeySource,
    runs: Mutex<HashMap<String, Arc<RunCtl>>>,
}

enum Halt {
    Stop(String),
}

impl Runner {
    pub fn new(store: Arc<Store>, notify: Arc<dyn Notify>, key: KeySource) -> Arc<Runner> {
        Arc::new(Runner { store, notify, key, runs: Mutex::new(HashMap::new()) })
    }

    fn ctl(&self, project_id: &str) -> Option<Arc<RunCtl>> {
        self.runs.lock().unwrap().get(project_id).cloned()
    }

    pub fn state(&self, project_id: &str) -> RunState {
        self.ctl(project_id).map(|c| c.get()).unwrap_or(RunState::Idle)
    }

    pub fn states(&self) -> HashMap<String, RunState> {
        self.runs.lock().unwrap().iter().map(|(k, v)| (k.clone(), v.get())).collect()
    }

    /// Starts the queue, or resumes it if paused.
    pub fn start(self: &Arc<Self>, project_id: &str) -> AppResult<RunState> {
        let project = self.store.project(project_id)?;
        let mut runs = self.runs.lock().unwrap();
        if let Some(ctl) = runs.get(project_id) {
            match ctl.get() {
                RunState::Paused => {
                    ctl.state.send_replace(RunState::Running);
                    self.notify.run(project_id, &RunState::Running);
                    self.store.event(Some(project_id), None, "run", "Resumed", None)?;
                    return Ok(RunState::Running);
                }
                RunState::Running => return Ok(RunState::Running),
                // A run that is winding down will exit on its own; start a fresh one beside it.
                _ => {}
            }
        }
        let (tx, _) = watch::channel(RunState::Running);
        let ctl = Arc::new(RunCtl { state: tx, concurrency: AtomicUsize::new(project.concurrency.clamp(1, 8) as usize) });
        runs.insert(project_id.to_string(), ctl.clone());
        drop(runs);
        self.store.event(Some(project_id), None, "run", "Started", None)?;
        self.notify.run(project_id, &RunState::Running);
        let me = self.clone();
        let pid = project_id.to_string();
        tauri::async_runtime::spawn(async move { me.supervise(pid, ctl).await });
        Ok(RunState::Running)
    }

    pub fn pause(&self, project_id: &str) -> RunState {
        if let Some(ctl) = self.ctl(project_id) {
            if ctl.get() == RunState::Running {
                ctl.state.send_replace(RunState::Paused);
                let _ = self.store.event(Some(project_id), None, "run", "Paused", None);
            }
            let s = ctl.get();
            self.notify.run(project_id, &s);
            return s;
        }
        RunState::Idle
    }

    pub fn stop(&self, project_id: &str) -> RunState {
        if let Some(ctl) = self.ctl(project_id) {
            ctl.state.send_replace(RunState::Stopping);
            let _ = self.store.event(Some(project_id), None, "run", "Stopping after the images in flight", None);
            self.notify.run(project_id, &RunState::Stopping);
            return RunState::Stopping;
        }
        RunState::Idle
    }

    pub fn set_concurrency(&self, project_id: &str, n: usize) {
        if let Some(ctl) = self.ctl(project_id) {
            ctl.concurrency.store(n.clamp(1, MAX_WORKERS), Ordering::SeqCst);
        }
    }

    async fn supervise(self: Arc<Self>, project_id: String, ctl: Arc<RunCtl>) {
        let mut set = tokio::task::JoinSet::new();
        for idx in 0..MAX_WORKERS {
            let me = self.clone();
            let ctl = ctl.clone();
            let pid = project_id.clone();
            set.spawn(async move { me.worker(idx, pid, ctl).await });
        }
        while set.join_next().await.is_some() {}
        let end = match ctl.get() {
            RunState::Stopped(r) => RunState::Stopped(r),
            RunState::Stopping => RunState::Stopped("Stopped".into()),
            _ => RunState::Idle,
        };
        {
            let mut runs = self.runs.lock().unwrap();
            if runs.get(&project_id).is_some_and(|c| Arc::ptr_eq(c, &ctl)) {
                runs.remove(&project_id);
            }
        }
        let message = match &end {
            RunState::Stopped(r) => format!("Run stopped: {r}"),
            _ => "Run finished: the queue is empty".to_string(),
        };
        let _ = self.store.event(Some(&project_id), None, "run", &message, None);
        // Report Idle with the reason attached so the UI can show why it stopped.
        self.notify.run(&project_id, &end);
    }

    async fn worker(&self, idx: usize, project_id: String, ctl: Arc<RunCtl>) {
        let mut rx = ctl.state.subscribe();
        loop {
            match ctl.get() {
                RunState::Paused => {
                    if rx.changed().await.is_err() {
                        return;
                    }
                    continue;
                }
                RunState::Running => {}
                _ => return,
            }
            if self.store.count_status(&project_id, "queued").unwrap_or(0) == 0 {
                return;
            }
            if idx >= ctl.concurrency.load(Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(400)).await;
                continue;
            }
            let Ok(project) = self.store.project(&project_id) else { return };
            if project.budget_usd > 0.0 && self.store.spent(&project_id).unwrap_or(0.0) >= project.budget_usd {
                ctl.stop(format!("Budget of ${:.2} reached", project.budget_usd));
                return;
            }
            let item = match self.store.claim_next(&project_id) {
                Ok(Some(item)) => item,
                _ => return,
            };
            self.notify.item(&item);
            if let Err(Halt::Stop(reason)) = self.process(&project, &item).await {
                let _ = self.store.set_status(&item.id, "queued", None);
                let _ = self.store.event(Some(&project_id), Some(&item.id), "limit", &reason, None);
                ctl.stop(reason);
            }
            if let Ok(fresh) = self.store.item(&item.id) {
                self.notify.item(&fresh);
            }
            if let Ok(spent) = self.store.spent(&project_id) {
                self.notify.spend(&project_id, spent);
            }
        }
    }

    fn prices(&self) -> PriceTable {
        // Built in, not user-editable: spend reporting should match OpenAI's published prices.
        pricing::defaults()
    }

    fn read_png(path: &str) -> Option<(String, Vec<u8>)> {
        let bytes = std::fs::read(path).ok()?;
        let name = std::path::Path::new(path).file_name()?.to_string_lossy().to_string();
        Some((name, bytes))
    }

    /// Up to three references, rotated per item so a big set does not converge on one anchor.
    fn references_for(&self, project: &Project, item: &Item) -> Vec<(String, Vec<u8>)> {
        if !project.use_references {
            return vec![];
        }
        let refs = self.store.references(&project.id).unwrap_or_default();
        if refs.is_empty() {
            return vec![];
        }
        let start = item.id.bytes().fold(7u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64)) as usize % refs.len();
        (0..refs.len().min(3))
            .filter_map(|i| Self::read_png(&refs[(start + i) % refs.len()].file))
            .collect()
    }

    async fn process(&self, project: &Project, item: &Item) -> Result<(), Halt> {
        let key = match (self.key)() {
            Ok(Some(k)) => k,
            Ok(None) => return Err(Halt::Stop("No API key. Add one in Settings.".into())),
            Err(e) => return Err(Halt::Stop(format!("Could not read the API key: {e}"))),
        };
        let base = self.store.setting("api_base").ok().flatten().unwrap_or_else(|| DEFAULT_BASE.to_string());
        let client = OpenAi::new(&base, key);
        let base_prompt = full_prompt(project, item);

        let (kind, prompt, images) = match (&item.redo_note, &item.redo_from) {
            (Some(note), Some(from)) => {
                let src = self.store.attempt(from).ok().and_then(|a| a.file).and_then(|f| Self::read_png(&f));
                match src {
                    Some(src) => ("edit", format!("{REDO_LEAD}\n\nChange: {note}\n\nOriginal brief, for context:\n{base_prompt}"), vec![src]),
                    None => ("redo", format!("{base_prompt}\n\nAdditional direction: {note}"), self.references_for(project, item)),
                }
            }
            (Some(note), None) => ("redo", format!("{base_prompt}\n\nAdditional direction: {note}"), self.references_for(project, item)),
            _ => {
                let refs = self.references_for(project, item);
                let prompt = if refs.is_empty() { base_prompt.clone() } else { format!("{STYLE_LEAD}\n\n{base_prompt}") };
                ("generate", prompt, refs)
            }
        };
        let req = ImageRequest {
            model: project.image_model.clone(),
            prompt: prompt.clone(),
            size: project.size.clone(),
            quality: project.quality.clone(),
            background: project.background.clone(),
        };

        let started = Instant::now();
        let mut result = Err(ApiError::Transient("not attempted".into()));
        for attempt in 0..4u32 {
            result = if images.is_empty() { client.generate(&req).await } else { client.edit(&req, &images).await };
            match &result {
                Err(e) if e.retryable() && attempt < 3 => {
                    let base_wait = if matches!(e, ApiError::RateLimited(_)) { 20 } else { 3 };
                    let jitter = rand::random::<u64>() % 2000;
                    tokio::time::sleep(Duration::from_millis(base_wait * 1000 * (attempt as u64 + 1) + jitter)).await;
                }
                _ => break,
            }
        }
        let ms = started.elapsed().as_millis() as i64;
        let note = item.redo_note.as_deref();

        let res = match result {
            Ok(res) => res,
            Err(e) => {
                let text = e.to_string();
                let _ = self.store.insert_attempt(item, kind, &req.model, &req.quality, &prompt, note, None, 0.0, None, None, None, Some(&text), ms);
                if e.stops_run() {
                    return Err(Halt::Stop(text));
                }
                let status = if matches!(e, ApiError::Refused(_)) { "refused" } else { "failed" };
                let _ = self.store.set_status(&item.id, status, Some(&text));
                let _ = self.store.event(Some(&project.id), Some(&item.id), "error", &format!("{}: {text}", item.slug), None);
                return Ok(());
            }
        };

        let cost = pricing::cost(&self.prices(), &req.model, &res.usage);
        let usage = serde_json::to_value(&res.usage).ok();
        let img = match image::load_from_memory(&res.png) {
            Ok(img) => img,
            Err(e) => {
                let text = format!("The returned image could not be decoded: {e}");
                let _ = self.store.insert_attempt(item, kind, &req.model, &req.quality, &prompt, note, None, cost, usage.as_ref(), None, None, Some(&text), ms);
                let _ = self.store.set_status(&item.id, "failed", Some(&text));
                return Ok(());
            }
        };
        let report = gates::evaluate(&img, GateMode::parse(&project.gate));
        let dir = self.store.project_dir(&project.id).join("images");
        let file = dir.join(format!("{}-{}.png", item.slug, new_id()));
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&file, &res.png)) {
            return Err(Halt::Stop(format!("Could not save images to disk: {e}")));
        }
        let file_str = file.to_string_lossy().to_string();
        let verdict = match report.verdict {
            Verdict::Pass => "pass",
            Verdict::Flag => "flag",
            Verdict::Fail => "fail",
        };
        let gate_json = serde_json::to_value(&report).ok();
        let saved = self
            .store
            .insert_attempt(item, kind, &req.model, &req.quality, &prompt, note, Some(&file_str), cost, usage.as_ref(), gate_json.as_ref(), Some(verdict), None, ms);
        let _ = self.store.clear_redo(&item.id);
        let reasons = report.reasons.join(" ");

        match report.verdict {
            Verdict::Pass => {
                let _ = self.store.set_status(&item.id, "done", None);
            }
            Verdict::Flag => {
                let _ = self.store.set_status(&item.id, "flagged", Some(&reasons));
            }
            Verdict::Fail if item.auto_retries == 0 => {
                // One automatic second try. A solid background gets a targeted edit; any other
                // defect is redrawn from the same prompt, which usually clears a one-off.
                let _ = self.store.bump_auto_retry(&item.id);
                if report.solid && project.background == "transparent" {
                    if let Ok(a) = &saved {
                        let _ = self.store.requeue_with(&item.id, Some(BG_FIX), Some(&a.id));
                    }
                } else {
                    let _ = self.store.requeue_with(&item.id, None, None);
                }
                let _ = self.store.event(Some(&project.id), Some(&item.id), "gate", &format!("{} failed the check, retrying once: {reasons}", item.slug), None);
            }
            Verdict::Fail => {
                let _ = self.store.set_status(&item.id, "flagged", Some(&reasons));
                let _ = self.store.event(Some(&project.id), Some(&item.id), "gate", &format!("{} needs review: {reasons}", item.slug), None);
            }
        }
        Ok(())
    }
}
