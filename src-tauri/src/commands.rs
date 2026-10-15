//! Every call the UI can make. API keys go in through `key_save` and never come back out.

use crate::error::{msg, AppResult};
use crate::export::{self, ExportReport};
use crate::planner::{self, PlanRequest};
use crate::pricing;
use crate::providers::{self, Kind, Provider, ProviderInfo};
use crate::runner::{base_url, RunState, Runner};
use crate::secrets::{self, KeyStatus};
use crate::spec::{self, SpecItem};
use crate::store::{
    Attempt, Event, Item, Project, ProjectPatch, ProjectStats, ProjectSummary, Reference,
    SpendSummary, Store,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub store: Arc<Store>,
    pub runner: Arc<Runner>,
}

type S<'a> = State<'a, AppState>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub default_provider: String,
    pub default_image_model: String,
    pub default_text_provider: String,
    pub default_text_model: String,
    pub default_concurrency: i64,
    pub default_budget_usd: f64,
    /// Per-provider base URL overrides; empty means the provider's default.
    pub base_urls: BTreeMap<String, String>,
}

fn settings(store: &Store) -> AppResult<Settings> {
    let get = |k: &str, d: &str| store.setting(k).map(|v| v.unwrap_or_else(|| d.to_string()));
    let mut base_urls = BTreeMap::new();
    for k in Kind::ALL {
        base_urls.insert(k.id().to_string(), base_url(store, k).unwrap_or_default());
    }
    Ok(Settings {
        default_provider: get("default_provider", "openai")?,
        default_image_model: get("default_image_model", "gpt-image-1")?,
        default_text_provider: get("default_text_provider", "openai")?,
        default_text_model: get("default_text_model", "gpt-4.1-mini")?,
        default_concurrency: get("default_concurrency", "3")?.parse().unwrap_or(3),
        default_budget_usd: get("default_budget_usd", "5")?.parse().unwrap_or(5.0),
        base_urls,
    })
}

fn provider(store: &Store, id: &str) -> AppResult<Provider> {
    let kind = Kind::parse(id);
    let key = match secrets::get_key(kind.id())? {
        Some(k) => k,
        None if kind == Kind::Compatible => String::new(),
        None => {
            return Err(msg(format!(
                "Add your {} API key in Settings first.",
                name_of(kind)
            )))
        }
    };
    let base = base_url(store, kind);
    if kind == Kind::Compatible && base.is_none() {
        return Err(msg(
            "Set the base URL for the OpenAI-compatible provider in Settings.",
        ));
    }
    Ok(Provider::new(kind, key, base))
}

fn name_of(kind: Kind) -> &'static str {
    providers::catalog()
        .into_iter()
        .find(|p| p.id == kind.id())
        .map(|p| p.name)
        .unwrap_or("provider")
}

// ---------- providers, keys and settings ----------

#[tauri::command]
pub fn provider_catalog() -> Vec<ProviderInfo> {
    providers::catalog()
}

#[tauri::command]
pub fn key_statuses() -> AppResult<BTreeMap<String, KeyStatus>> {
    secrets::status_all()
}

/// Verifies a key against the provider before storing it.
#[tauri::command]
pub async fn key_save(state: S<'_>, provider: String, key: String) -> AppResult<KeyStatus> {
    let kind = Kind::parse(&provider);
    let key = key.trim().to_string();
    Provider::new(kind, key.clone(), base_url(&state.store, kind))
        .verify()
        .await?;
    secrets::set_key(kind.id(), &key)?;
    if kind == Kind::Openai && state.store.setting("default_image_model")?.is_none() {
        if let Ok(models) = Provider::new(kind, key, base_url(&state.store, kind))
            .list_models()
            .await
        {
            let pick = |prefs: &[&str]| {
                prefs
                    .iter()
                    .find(|p| models.iter().any(|m| m == *p))
                    .map(|s| s.to_string())
            };
            if let Some(m) = pick(&["gpt-image-1.5", "gpt-image-1", "gpt-image-1-mini"]) {
                state.store.set_setting("default_image_model", &m)?;
            }
            if let Some(m) = pick(&["gpt-5-mini", "gpt-4.1-mini", "gpt-4o-mini", "gpt-4.1"]) {
                state.store.set_setting("default_text_model", &m)?;
            }
        }
    }
    secrets::status(kind.id())
}

#[tauri::command]
pub fn key_delete(provider: String) -> AppResult<KeyStatus> {
    secrets::delete_key(&provider)?;
    secrets::status(&provider)
}

/// Models the account can see, for providers that list them; the catalog covers the rest.
#[tauri::command]
pub async fn models(state: S<'_>, provider: String) -> AppResult<Vec<String>> {
    Ok(self::provider(&state.store, &provider)?
        .list_models()
        .await?)
}

#[tauri::command]
pub fn settings_get(state: S<'_>) -> AppResult<Settings> {
    settings(&state.store)
}

#[tauri::command]
pub fn settings_save(state: S<'_>, settings: Settings) -> AppResult<Settings> {
    let st = &state.store;
    st.set_setting(
        "default_provider",
        Kind::parse(&settings.default_provider).id(),
    )?;
    st.set_setting("default_image_model", settings.default_image_model.trim())?;
    st.set_setting(
        "default_text_provider",
        Kind::parse(&settings.default_text_provider).id(),
    )?;
    st.set_setting("default_text_model", settings.default_text_model.trim())?;
    st.set_setting(
        "default_concurrency",
        &settings.default_concurrency.clamp(1, 8).to_string(),
    )?;
    st.set_setting(
        "default_budget_usd",
        &settings.default_budget_usd.max(0.0).to_string(),
    )?;
    for (id, url) in &settings.base_urls {
        let url = url.trim().trim_end_matches('/');
        let local = url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
        if !url.is_empty() && !url.starts_with("https://") && !local {
            return Err(msg(format!(
                "The {id} base URL must be https:// (or a localhost address)."
            )));
        }
        st.set_setting(&format!("base_url.{}", Kind::parse(id).id()), url)?;
    }
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
    pub caps: providers::Caps,
    pub estimate_usd: f64,
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
    let name = if name.trim().is_empty() {
        "Untitled project".to_string()
    } else {
        name
    };
    state.store.create_project_with(
        &name,
        &s.default_provider,
        &s.default_image_model,
        &s.default_text_provider,
        &s.default_text_model,
        s.default_concurrency,
        s.default_budget_usd,
    )
}

#[tauri::command]
pub fn project_get(state: S<'_>, id: String) -> AppResult<ProjectDetail> {
    let project = state.store.project(&id)?;
    let kind = Kind::parse(&project.provider);
    Ok(ProjectDetail {
        caps: providers::caps_for(kind, &project.image_model),
        estimate_usd: providers::estimate_for(kind, &project.image_model),
        stats: state.store.stats(&id)?,
        run: state.runner.state(&id),
        references: state.store.references(&id)?,
        project,
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
pub fn project_duplicate(state: S<'_>, id: String) -> AppResult<Project> {
    state.store.duplicate_project(&id)
}

#[tauri::command]
pub fn project_delete(state: S<'_>, id: String) -> AppResult<()> {
    if !matches!(
        state.runner.state(&id),
        RunState::Idle | RunState::Stopped(_)
    ) {
        return Err(msg("Stop the run before deleting this project."));
    }
    state.store.delete_project(&id)
}

// ---------- plan ----------

#[tauri::command]
pub async fn plan_draft(
    app: tauri::AppHandle,
    state: S<'_>,
    project_id: String,
    request: PlanRequest,
) -> AppResult<Project> {
    use tauri::Emitter;
    let store = state.store.clone();
    let project = store.project(&project_id)?;
    if project.spec_locked {
        return Err(msg("The spec is approved. Unlock it to re-plan."));
    }
    let planner_provider = provider(&store, &project.text_provider)?;
    // Stream the plan to the window as it is written, at most every 120 ms.
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let pid = project_id.clone();
    let mut on_text = |text: &str| {
        if last.elapsed() >= std::time::Duration::from_millis(120) {
            last = std::time::Instant::now();
            let _ = app.emit(
                "plan-stream",
                serde_json::json!({ "project_id": pid, "text": text }),
            );
        }
    };
    let (plan, usage) = planner::draft_streaming(
        &planner_provider,
        &project.text_model,
        &request,
        &mut on_text,
    )
    .await?;
    let cost = pricing::cost(&pricing::defaults(), &project.text_model, &usage);
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
    let items: Vec<(String, String, String)> = plan
        .items
        .into_iter()
        .map(|i| (i.slug, i.title, i.subject))
        .collect();
    store.replace_plan(&project_id, &items)?;
    store.event(
        Some(&project_id),
        None,
        "plan",
        &format!("Drafted a plan of {} images (${cost:.3})", items.len()),
        None,
    )?;
    store.project(&project_id)
}

#[tauri::command]
pub fn spec_lock(state: S<'_>, project_id: String, locked: bool) -> AppResult<Project> {
    state.store.lock_spec(&project_id, locked)
}

/// Imports a spec (.json) or a prompt list (.csv/.json array), replacing unstarted images.
#[tauri::command]
pub fn spec_import(state: S<'_>, project_id: String, path: String) -> AppResult<usize> {
    let project = state.store.project(&project_id)?;
    if project.spec_locked {
        return Err(msg("Unlock the spec before importing."));
    }
    let text = std::fs::read_to_string(&path)?;
    let file_name = std::path::Path::new(&path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();
    let spec = spec::parse(&text, &file_name)?;
    let mut patch = ProjectPatch {
        size: spec.size.clone(),
        background: spec.background.clone(),
        gate: spec.gate.clone(),
        ..Default::default()
    };
    if !spec.style_brief.is_empty() {
        patch.style_brief = Some(spec.style_brief.clone());
    }
    if !spec.shared_tail.is_empty() {
        patch.shared_tail = Some(spec.shared_tail.clone());
    }
    if !spec.idea.is_empty() {
        patch.idea = Some(spec.idea.clone());
    }
    if !spec.name.is_empty() && project.name == "Untitled project" {
        patch.name = Some(spec.name.clone());
    }
    state.store.update_project(&project_id, patch)?;
    let items: Vec<(String, String, String)> = spec
        .items
        .into_iter()
        .map(|i| (i.slug, i.title, i.subject))
        .collect();
    state.store.replace_plan(&project_id, &items)?;
    state.store.event(
        Some(&project_id),
        None,
        "plan",
        &format!("Imported {} images from {file_name}", items.len()),
        None,
    )?;
    Ok(items.len())
}

#[tauri::command]
pub fn spec_export(state: S<'_>, project_id: String, path: String) -> AppResult<()> {
    let project = state.store.project(&project_id)?;
    let items = state
        .store
        .items(&project_id)?
        .into_iter()
        .map(|i| SpecItem {
            slug: i.slug,
            title: i.title,
            subject: i.subject,
        })
        .collect();
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&spec::export(&project, items))?,
    )?;
    Ok(())
}

// ---------- items ----------

#[tauri::command]
pub fn items(state: S<'_>, project_id: String) -> AppResult<Vec<Item>> {
    state.store.items(&project_id)
}

#[tauri::command]
pub fn item_add(
    state: S<'_>,
    project_id: String,
    title: String,
    subject: String,
) -> AppResult<Item> {
    if subject.trim().is_empty() {
        return Err(msg("Describe the image first."));
    }
    state.store.add_item(&project_id, &title, &subject)
}

#[tauri::command]
pub fn item_edit(
    state: S<'_>,
    id: String,
    title: Option<String>,
    subject: Option<String>,
) -> AppResult<Item> {
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
pub fn item_redo(
    state: S<'_>,
    id: String,
    note: Option<String>,
    edit_current: bool,
) -> AppResult<Item> {
    let item = state.store.item(&id)?;
    let note = note.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let from = if edit_current && note.is_some() {
        item.current_attempt.clone()
    } else {
        None
    };
    state
        .store
        .set_redo(&id, note.as_deref(), from.as_deref())?;
    let what = match &note {
        Some(n) => format!("Redo {} with note: {n}", item.slug),
        None => format!("Redo {}", item.slug),
    };
    state
        .store
        .event(Some(&item.project_id), Some(&id), "review", &what, None)?;
    state.runner.start(&item.project_id)?;
    state.store.item(&id)
}

// ---------- runs ----------

#[tauri::command]
pub fn run_start(state: S<'_>, project_id: String) -> AppResult<RunState> {
    let project = state.store.project(&project_id)?;
    if !project.spec_locked {
        return Err(msg(
            "Approve the spec before running it. Use Preview to test a few first.",
        ));
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
    state
        .store
        .add_reference(&a.project_id, &file, Some(&attempt_id))
}

#[tauri::command]
pub fn reference_import(state: S<'_>, project_id: String, path: String) -> AppResult<Reference> {
    let img = image::open(&path)?;
    let dir = state.store.project_dir(&project_id).join("refs");
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{}.png", crate::store::new_id()));
    img.save(&dest)?;
    state
        .store
        .add_reference(&project_id, &dest.to_string_lossy(), None)
}

#[tauri::command]
pub fn reference_remove(state: S<'_>, id: String) -> AppResult<()> {
    state.store.remove_reference(&id)
}

// ---------- export and activity ----------

#[tauri::command]
pub async fn export_project(
    state: S<'_>,
    project_id: String,
    only_approved: bool,
) -> AppResult<ExportReport> {
    let store = state.store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let project = store.project(&project_id)?;
        let dir = project
            .output_dir
            .clone()
            .ok_or_else(|| msg("Choose an output folder first."))?;
        export::export(&store, &project, std::path::Path::new(&dir), only_approved)
    })
    .await
    .map_err(|e| msg(e.to_string()))?
}

/// Shows a project's output folder, or an image file, in the system file manager.
#[tauri::command]
pub fn reveal(path: String) -> AppResult<()> {
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(msg("That file or folder no longer exists."));
    }
    tauri_plugin_opener::reveal_item_in_dir(p).map_err(|e| msg(e.to_string()))
}

/// Opens a provider's API key page in the browser. Only catalog URLs can be opened.
#[tauri::command]
pub fn open_key_page(provider: String) -> AppResult<()> {
    let url = providers::catalog()
        .into_iter()
        .find(|p| p.id == provider && !p.key_url.is_empty())
        .map(|p| p.key_url)
        .ok_or_else(|| msg("No key page for that provider."))?;
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(|e| msg(e.to_string()))
}

#[tauri::command]
pub fn events(state: S<'_>, project_id: String, limit: Option<i64>) -> AppResult<Vec<Event>> {
    state.store.events(&project_id, limit.unwrap_or(300))
}
