//! Tauri application shell for Fedora Mirror Manager.
//!
//! Thin adapter: window setup, command registration and mapping between the
//! frontend and the domain crate. All domain logic lives in `fmm-core`.

mod commands;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(commands::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_status,
            commands::list_repos,
            commands::list_snapshots,
            commands::preview_action,
            commands::preview_restore,
            commands::apply_plan,
            commands::apply_restore,
            commands::helper_status,
            commands::install_helper
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
