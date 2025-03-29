// src/lib.rs

use tauri::{Manager, State};

mod mongodb_installer;
mod mongodb_manager;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Initialize MongoDB state with database name
            let mongodb_state = mongodb_manager::MongoDbState::new("app_database");
            app.manage(mongodb_state);

            // Auto-connect if MongoDB is installed
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if mongodb_installer::is_mongodb_installed().await {
                    let state: State<'_, mongodb_manager::MongoDbState> = app_handle.state();
                    if let Err(e) = mongodb_manager::auto_connect(&state).await {
                        eprintln!("Auto-connect failed: {}", e);
                    }
                }
            });            

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            // MongoDB installation commands
            mongodb_installer::is_mongodb_installed,
            mongodb_installer::install_mongodb,
            
            // MongoDB database operations
            mongodb_manager::connect_mongodb,
            mongodb_manager::disconnect_mongodb,
            mongodb_manager::insert_document,
            mongodb_manager::find_documents,
            mongodb_manager::update_document,
            mongodb_manager::delete_document,
            mongodb_manager::list_collections,
            
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}