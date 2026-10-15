//! Every call the UI can make. The API key goes in through `key_save` and never comes back out.

use crate::error::{msg, AppResult};
use crate::export::{self, ExportReport};
use crate::openai::{OpenAi, DEFAULT_BASE};
use crate::planner::{self, PlanRequest};
use crate::pricing::{self, PriceTable};
use crate::runner::{RunState, Runner};
use crate::secrets::{self, KeyStatus};
use crate::store::{Attempt, Event, Item, Project, ProjectPatch, ProjectStats, ProjectSummary, Reference, SpendSummary, Store};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub store: Arc<Store>,
    pub runner: Arc<Runner>,
}

type S<'a> = State<'a, AppState>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_image_model: String,
    pub default_text_model: String,
    pub default_concurrency: i64,
    pub default_budget_usd: f64,
    pub api_base: String,
    pub prices: PriceTable,
}

fn settings(store: &Store) -> AppResult<Settings> {
    let get = |k: &str, d: &str| store.setting(k).map(|v| v.unwrap_or_else(|| d.to_string()));
    Ok(Settings {
        default_image_model: get("default_image_model", "gpt-image-1")?,
        default_text_model: get("default_text_model", "gpt-4.1-mini")?,
        default_concurrency: get("default_concurrency", "3")?.parse().unwrap_or(3),
        default_budget_usd: get("default_budget_usd", "5")?.parse().unwrap_or(5.0),
        api_base: get("api_base", DEFAULT_BASE)?,
        prices: pricing::defaults(),
    })
}

fn client(store: &Store) -> AppResult<OpenAi> {
    let key = secrets::get_key()?.ok_or_else(|| msg("Add your OpenAI API key in Settings first."))?;
    Ok(OpenAi::new(&settings(store)?.api_base, key))
}

// ---------- key and settings ----------

#[tauri::command]
pub fn key_status() -> AppResult<KeyStatus> {
    secrets::status()
}

/// Checks the key against the API before storing it, and picks sensible default models.
#[tauri::command]
pub async fn key_save(state: S<'_>, key: String) -> AppResult<KeyStatus> {
    let key = key.trim().to_string();
    let base = settings(&state.store)?.api_base;
    let models = OpenAi::new(&base, key.clone()).list_models().await?;
    secrets::set_key(&key)?;
    let pick = |prefs: &[&str]| prefs.iter().find(|p| models.iter().any(|m| m == *p)).map(|s| s.to_string());
    if state.store.setting("default_image_model")?.is_none() {
        if let Some(m) = pick(&["gpt-image-1.5", "gpt-image-1", "gpt-image-1-mini"]) {
            state.store.set_setting("default_image_model", &m)?;
        }
    }
    if state.store.setting("default_text_model")?.is_none() {
        if let Some(m) = pick(&["gpt-5-mini", "gpt-4.1-mini", "gpt-4o-mini", "gpt-4.1"]) {
            state.store.set_setting("default_text_model", &m)?;
        }
    }
    secrets::status()
}

#[tauri::command]
pub fn key_delete() -> AppResult<KeyStatus> {
    secrets::delete_key()?;
    secrets::status()
}

#[derive(Serialize)]
pub struct ModelLists {
    pub image: Vec<String>,
    pub text: Vec<String>,
}

#[tauri::command]
pub async fn models(state: S<'_>) -> AppResult<ModelLists> {
    let all = client(&state.store)?.list_models().await?;
    let image = all.iter().filter(|m| m.contains("image") || m.starts_with("dall-e")).cloned().collect();
    let skip = ["image", "audio", "realtime", "tts", "transcribe", "search", "embedding", "moderation", "dall-e", "codex", "computer"];
    let text = all
        .iter()
        .filter(|m| (m.starts_with("gpt-") || m.starts_with('o')) && !skip.iter().any(|s| m.contains(s)))
        .cloned()
        .collect();
    Ok(ModelLists { image, text })
}

#[tauri::command]
pub fn settings_get(state: S<'_>) -> AppResult<Settings> {
    settings(&state.store)
}

#[tauri::command]
pub fn settings_save(state: S<'_>, settings: Settings) -> AppResult<Settings> {
    let st = &state.store;
    st.set_setting("default_image_model", &settings.default_image_model)?;
    st.set_setting("default_text_model", &settings.default_text_model)?;
    st.set_setting("default_concurrency", &settings.default_concurrency.clamp(1, 8).to_string())?;
    st.set_setting("default_budget_usd", &settings.default_budget_usd.max(0.0).to_string())?;
    let base = settings.api_base.trim().trim_end_matches('/');
    if !base.starts_with("https://") && !base.starts_with("http://127.0.0.1") && !base.starts_with("http://localhost") {
        return Err(msg("The API base must be an https:// URL."));
    }
    st.set_setting("api_base", base)?;
    self::settings(st)
}

#[tauri::command]
pub fn spend(state: S<'_>) -> AppResult<SpendSummary> {
    state.store.spend_summary()
}

// ---------- projects ----------

#[derive(Serialize)]
pub struct ProjectDetail {
    pub project: Project,
    pub stats: ProjectStats,
    pub run: RunState,
    pub references: Vec<Reference>,
}

#[tauri::command]
pub fn projects(state: S<'_>) -> AppResult<Vec<ProjectSummary>> {
    state.store.projects()
}

#[tauri::command]
pub fn run_states(state: S<'_>) -> HashMap<String, RunState> {
    state.runner.states()
}

#[tauri::command]
pub fn project_create(state: S<'_>, name: String) -> AppResult<Project> {
    let s = settings(&state.store)?;
    let name = if name.trim().is_empty() { "Untitled project".to_string() } else { name };
    state.store.create_project(&name, &s.default_image_model, &s.default_text_model, s.default_concurrency, s.default_budget_usd)
}

#[tauri::command]
pub fn project_get(state: S<'_>, id: String) -> AppResult<ProjectDetail> {
    Ok(ProjectDetail {
        project: state.store.project(&id)?,
        stats: state.store.stats(&id)?,
        run: state.runner.state(&id),
        references: state.store.references(&id)?,
    })
}

#[tauri::command]
pub fn project_update(state: S<'_>, id: String, patch: ProjectPatch) -> AppResult<Project> {
    if let Some(n) = patch.concurrency {
        state.runner.set_concurrency(&id, n as usize);
    }
    state.store.update_project(&id, patch)
}

#[tauri::command]
pub fn project_delete(state: S<'_>, id: String) -> AppResult<()> {
    if !matches!(state.runner.state(&id), RunState::Idle | RunState::Stopped(_)) {
        return Err(msg("Stop the run before deleting this project."));
    }
    state.store.delete_project(&id)
}

// ---------- plan ----------

#[tauri::command]
pub async fn plan_draft(app: tauri::AppHandle, state: S<'_>, project_id: String, request: PlanRequest) -> AppResult<Project> {
    use tauri::Emitter;
    let store = state.store.clone();
    let project = store.project(&project_id)?;
    if project.spec_locked {
        return Err(msg("The spec is approved. Unlock it to re-plan."));
    }
    let client = client(&store)?;
    // Stream the plan to the window as it is written, at most every 120 ms.
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let pid = project_id.clone();
    let mut on_text = |text: &str| {
        if last.elapsed() >= std::time::Duration::from_millis(120) {
            last = std::time::Instant::now();
            let _ = app.emit("plan-stream", serde_json::json!({ "project_id": pid, "text": text }));
        }
    };
    let (plan, usage) = planner::draft_streaming(&client, &project.text_model, &request, &mut on_text).await?;
    let cost = pricing::cost(&settings(&store)?.prices, &project.text_model, &usage);
    store.record_plan_call(&project_id, &project.text_model, cost)?;
    let rename = project.name == "Untitled project";
    store.update_project(
        &project_id,
        ProjectPatch {
            name: rename.then(|| plan.name.clone()),
            idea: Some(request.idea.clone()),
            style_brief: Some(plan.style_brief.clone()),
            shared_tail: Some(plan.shared_tail.clone()),
            background: Some(request.background.clone()),
            gate: Some(request.gate.clone()),
            size: Some(request.size.clone()),
            ..Default::default()
        },
    )?;
    let items: Vec<(String, String, String)> = plan.items.into_iter().map(|i| (i.slug, i.title, i.subject)).collect();
    store.replace_plan(&project_id, &items)?;
    store.event(Some(&project_id), None, "plan", &format!("Drafted a plan of {} images (${cost:.3})", items.len()), None)?;
    store.project(&project_id)
}

#[tauri::command]
pub fn spec_lock(state: S<'_>, project_id: String, locked: bool) -> AppResult<Project> {
    state.store.lock_spec(&project_id, locked)
}

// ---------- items ----------

#[tauri::command]
pub fn items(state: S<'_>, project_id: String) -> AppResult<Vec<Item>> {
    state.store.items(&project_id)
}

#[tauri::command]
pub fn item_add(state: S<'_>, project_id: String, title: String, subject: String) -> AppResult<Item> {
    if subject.trim().is_empty() {
        return Err(msg("Describe the image first."));
    }
    state.store.add_item(&project_id, &title, &subject)
}

#[tauri::command]
pub fn item_edit(state: S<'_>, id: String, title: Option<String>, subject: Option<String>) -> AppResult<Item> {
    state.store.edit_item(&id, title, subject)
}

#[tauri::command]
pub fn item_delete(state: S<'_>, id: String) -> AppResult<()> {
    state.store.delete_item(&id)
}

#[tauri::command]
pub fn attempts(state: S<'_>, item_id: String) -> AppResult<Vec<Attempt>> {
    state.store.attempts(&item_id)
}

#[tauri::command]
pub fn item_approve(state: S<'_>, id: String, attempt_id: Option<String>) -> AppResult<Item> {
    state.store.approve(&id, attempt_id.as_deref())
}

#[tauri::command]
pub fn item_reject(state: S<'_>, id: String) -> AppResult<Item> {
    state.store.reject(&id)
}

/// Redo one image. With `edit_current`, the note is applied as an edit to the current image;
/// otherwise the image is redrawn from scratch with the note appended.
#[tauri::command]
pub fn item_redo(state: S<'_>, id: String, note: Option<String>, edit_current: bool) -> AppResult<Item> {
    let item = state.store.item(&id)?;
    let note = note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let from = if edit_current && note.is_some() { item.current_attempt.clone() } else { None };
    state.store.set_redo(&id, note.as_deref(), from.as_deref())?;
    let what = match &note {
        Some(n) => format!("Redo {} with note: {n}", item.slug),
        None => format!("Redo {}", item.slug),
    };
    state.store.event(Some(&item.project_id), Some(&id), "review", &what, None)?;
    state.runner.start(&item.project_id)?;
    state.store.item(&id)
}

// ---------- runs ----------

#[tauri::command]
pub fn run_start(state: S<'_>, project_id: String) -> AppResult<RunState> {
    let project = state.store.project(&project_id)?;
    if !project.spec_locked {
        return Err(msg("Approve the spec before running it. Use Preview to test a few first."));
    }
    state.store.enqueue(&project_id, None, 0)?;
    state.runner.start(&project_id)
}

#[tauri::command]
pub fn run_preview(state: S<'_>, project_id: String, item_ids: Vec<String>) -> AppResult<RunState> {
    if item_ids.is_empty() {
        return Err(msg("Pick at least one image to preview."));
    }
    state.store.enqueue(&project_id, Some(&item_ids), 10)?;
    state.runner.start(&project_id)
}

#[tauri::command]
pub fn run_pause(state: S<'_>, project_id: String) -> RunState {
    state.runner.pause(&project_id)
}

#[tauri::command]
pub fn run_stop(state: S<'_>, project_id: String) -> RunState {
    state.runner.stop(&project_id)
}

// ---------- references ----------

#[tauri::command]
pub fn reference_from_attempt(state: S<'_>, attempt_id: String) -> AppResult<Reference> {
    let a = state.store.attempt(&attempt_id)?;
    let file = a.file.ok_or_else(|| msg("That attempt has no image."))?;
    state.store.add_reference(&a.project_id, &file, Some(&attempt_id))
}

#[tauri::command]
pub fn reference_import(state: S<'_>, project_id: String, path: String) -> AppResult<Reference> {
    let img = image::open(&path)?;
    let dir = state.store.project_dir(&project_id).join("refs");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{}.png", crate::store::new_id()));
    img.save(&dest)?;
    state.store.add_reference(&project_id, &dest.to_string_lossy(), None)
}

#[tauri::command]
pub fn reference_remove(state: S<'_>, id: String) -> AppResult<()> {
    state.store.remove_reference(&id)
}

// ---------- export and activity ----------

#[tauri::command]
pub async fn export_project(state: S<'_>, project_id: String, only_approved: bool) -> AppResult<ExportReport> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = store.project(&project_id)?;
        let dir = project.output_dir.clone().ok_or_else(|| msg("Choose an output folder first."))?;
        export::export(&store, &project, std::path::Path::new(&dir), only_approved)
    })
    .await
    .map_err(|e| msg(e.to_string()))?
}

#[tauri::command]
pub fn events(state: S<'_>, project_id: String, limit: Option<i64>) -> AppResult<Vec<Event>> {
    state.store.events(&project_id, limit.unwrap_or(300))
}
