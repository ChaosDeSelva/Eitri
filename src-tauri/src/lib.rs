pub mod bases;
pub mod commands;
pub mod error;
pub mod export;
pub mod gates;
pub mod keying;
pub mod openai;
pub mod planner;
pub mod pricing;
pub mod providers;
pub mod runner;
pub mod secrets;
pub mod spec;
pub mod store;

use commands::AppState;
use runner::{Notify, RunState, Runner};
use serde_json::json;
use std::sync::Arc;
use store::{Item, Store};
use tauri::{AppHandle, Emitter, Manager};

/// Forwards runner progress to the window as events.
struct TauriNotify(AppHandle);

impl Notify for TauriNotify {
    fn item(&self, item: &Item) {
        let _ = self.0.emit("item-updated", item);
    }
    fn run(&self, project_id: &str, state: &RunState) {
        let _ = self.0.emit(
            "run-updated",
            json!({ "project_id": project_id, "run": state }),
        );
        // A desktop notification when a run ends, so a long batch can run in the background.
        let body = match state {
            RunState::Idle => "Finished: the queue is empty.".to_string(),
            RunState::Stopped(reason) => format!("Stopped: {reason}"),
            _ => return,
        };
        let name = self
            .0
            .try_state::<AppState>()
            .and_then(|s| s.store.project(project_id).ok())
            .map(|p| p.name)
            .unwrap_or_else(|| "Eitri".into());
        use tauri_plugin_notification::NotificationExt;
        let _ = self
            .0
            .notification()
            .builder()
            .title(name)
            .body(body)
            .show();
    }
    fn spend(&self, project_id: &str, spent_usd: f64) {
        let _ = self.0.emit(
            "spend-updated",
            json!({ "project_id": project_id, "spent_usd": spent_usd }),
        );
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let data_dir = app.path().app_data_dir()?;
            let store = Arc::new(Store::open(&data_dir)?);
            let notify: Arc<dyn Notify> = Arc::new(TauriNotify(app.handle().clone()));
            let runner = Runner::new(
                store.clone(),
                notify,
                Arc::new(|p: &str| secrets::get_key(p)),
            );
            app.manage(AppState { store, runner });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::provider_catalog,
            commands::key_statuses,
            commands::key_save,
            commands::key_delete,
            commands::models,
            commands::settings_get,
            commands::settings_save,
            commands::spend,
            commands::projects,
            commands::run_states,
            commands::project_create,
            commands::project_get,
            commands::project_update,
            commands::project_duplicate,
            commands::project_delete,
            commands::plan_draft,
            commands::spec_lock,
            commands::spec_import,
            commands::spec_export,
            commands::items,
            commands::item_add,
            commands::item_edit,
            commands::item_delete,
            commands::attempts,
            commands::item_approve,
            commands::item_reject,
            commands::item_redo,
            commands::run_start,
            commands::run_preview,
            commands::run_pause,
            commands::run_stop,
            commands::reference_from_attempt,
            commands::reference_import,
            commands::reference_remove,
            commands::export_project,
            commands::reveal,
            commands::open_key_page,
            commands::events,
            commands::bases_list,
            commands::base_get,
            commands::base_create,
            commands::base_update,
            commands::base_delete,
            commands::base_duplicate,
            commands::base_generate,
            commands::base_from_project,
            commands::base_apply,
            commands::base_export,
            commands::base_import,
            commands::base_ref_add,
            commands::base_ref_remove,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Eitri");
}
