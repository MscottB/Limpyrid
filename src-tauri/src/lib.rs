pub mod catalog;
pub mod clean;
pub mod commands;
pub mod containers;
pub mod drives;
pub mod scan;
pub mod worktrees;

use commands::{AppState, CategoryInfo, Entry};
use containers::{Engine, Usage};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};

// Corre trabajo bloqueante fuera del hilo de la interfaz; los errores son codigos.
async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, &'static str> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|_| "command_failed".to_string())?.map_err(String::from)
}

#[tauri::command]
fn start_scan(app: AppHandle, state: State<AppState>, root: String) -> Result<(), String> {
    commands::begin_scan(&state)?;
    std::thread::spawn(move || {
        let state = app.state::<AppState>();
        let start = Instant::now();
        let last = AtomicU64::new(0); // ms del ultimo evento enviado: evita inundar la interfaz
        let on_progress = |p: scan::Progress| {
            let now = start.elapsed().as_millis() as u64;
            if now.saturating_sub(last.load(Ordering::Relaxed)) >= 100 {
                last.store(now, Ordering::Relaxed);
                let _ = app.emit("scan-progress", p);
            }
        };
        let (cancelled, denied) = commands::run_scan(&state, &PathBuf::from(root), &on_progress);
        let _ = app.emit("scan-done", json!({ "cancelled": cancelled, "denied": denied }));
    });
    Ok(())
}

#[tauri::command]
fn cancel_scan(state: State<AppState>) {
    state.cancel.store(true, Ordering::SeqCst);
}

#[tauri::command]
fn children(state: State<AppState>, path: String) -> Result<Vec<Entry>, String> {
    Ok(commands::children(&state, &path)?)
}

#[tauri::command]
fn treemap(state: State<AppState>, path: String) -> Result<scan::Node, String> {
    Ok(commands::treemap(&state, &path)?)
}

#[tauri::command]
async fn categories(app: AppHandle) -> Result<Vec<CategoryInfo>, String> {
    blocking(move || Ok(commands::categories(&app.state::<AppState>(), catalog::CATALOG))).await
}

#[tauri::command]
async fn clean_category(app: AppHandle, id: String, items: Vec<String>, min_age_hours: Option<u64>) -> Result<clean::Report, String> {
    blocking(move || {
        let emit = |freed: u64, deleted: u64| {
            let _ = app.emit("clean-progress", json!({ "id": id, "freed": freed, "deleted": deleted }));
        };
        commands::clean_category_with(&app.state::<AppState>(), catalog::CATALOG, &id, &items, min_age_hours, &emit)
    })
    .await
}

#[tauri::command]
async fn trash_paths(app: AppHandle, paths: Vec<String>) -> Result<clean::Report, String> {
    blocking(move || commands::trash_paths(&app.state::<AppState>(), &paths)).await
}

#[tauri::command]
async fn worktrees(app: AppHandle, min_age_hours: Option<u64>) -> Result<worktrees::WorktreeList, String> {
    blocking(move || commands::list_worktrees(&app.state::<AppState>(), catalog::CATALOG, min_age_hours, commands::conversations_dir().as_deref())).await
}

#[tauri::command]
async fn remove_worktrees(app: AppHandle, paths: Vec<String>, min_age_hours: Option<u64>) -> Result<clean::Report, String> {
    blocking(move || {
        commands::remove_worktrees(&app.state::<AppState>(), catalog::CATALOG, &paths, min_age_hours, commands::conversations_dir().as_deref())
    })
    .await
}

#[tauri::command]
async fn container_usage(engine: Engine) -> Result<Usage, String> {
    blocking(move || Ok(containers::usage(engine))).await
}

// Prune y compactar comparten con limpiar y la papelera la marca de "trabajando" (commands::begin_work).
#[tauri::command]
async fn container_prune(app: AppHandle, engine: Engine, all_images: bool) -> Result<String, String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _work = commands::begin_work(&state)?;
        let out = containers::prune(engine, all_images);
        commands::refresh_containers(&state, catalog::CATALOG, engine_id(engine));
        out
    })
    .await
}

#[tauri::command]
async fn container_compact(app: AppHandle, engine: Engine) -> Result<(u64, u64), String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let _work = commands::begin_work(&state)?;
        let out = containers::compact(engine);
        commands::refresh_containers(&state, catalog::CATALOG, engine_id(engine));
        out
    })
    .await
}

// Fila del catalogo de cada motor.
fn engine_id(engine: Engine) -> &'static str {
    match engine {
        Engine::Docker => "docker",
        Engine::Podman => "podman",
    }
}

// Consultar un lector dormido o una red caida puede tardar segundos: fuera del hilo de la interfaz.
#[tauri::command]
async fn drives() -> Result<Vec<drives::Drive>, String> {
    blocking(|| Ok(drives::drives())).await
}

#[tauri::command]
fn denied_paths(state: State<AppState>) -> Vec<String> {
    commands::denied_paths(&state)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            start_scan,
            cancel_scan,
            children,
            treemap,
            categories,
            clean_category,
            trash_paths,
            worktrees,
            remove_worktrees,
            container_usage,
            container_prune,
            container_compact,
            drives,
            denied_paths
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
