// src/mongodb_installer/mod.rs

use tauri::AppHandle;
use std::env;

// Import OS-specific modules
mod ubuntu;
mod windows;

// Re-export shared types
pub use ubuntu::SudoPasswordRequest;

#[tauri::command]
pub async fn install_mongodb(app: AppHandle) -> Result<(), String> {
    let os = env::consts::OS;
    
    match os {
        "linux" => ubuntu::install_mongodb(app).await,
        "windows" => windows::install_mongodb(&app).await,
        _ => Err(format!("Unsupported operating system: {}", os)),
    }
}

#[tauri::command]
pub async fn is_mongodb_installed() -> bool {
    let os = env::consts::OS;
    
    match os {
        "linux" => ubuntu::is_mongodb_installed().await,
        "windows" => windows::is_mongodb_installed().await,
        _ => false, // Unsupported OS
    }
}