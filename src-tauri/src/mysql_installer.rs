// src/mysql_installer.rs

use tauri::{Manager, AppHandle};
use tauri_plugin_shell::process::CommandEvent;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::oneshot;
use tauri::Listener;
use tauri::Emitter;
use tauri_plugin_shell::ShellExt;

#[derive(Serialize, Deserialize, Clone)]
struct SudoPasswordRequest {
    request_id: String,
}

async fn get_sudo_password(app: &AppHandle) -> Result<String, anyhow::Error> {
    let (tx, rx) = oneshot::channel();
    let tx = Arc::new(tokio::sync::Mutex::new(Some(tx)));
    let request_id = Uuid::new_v4().to_string();

    println!("Requesting sudo password with request_id: {}", request_id);

    app.emit("sudo-password-request", SudoPasswordRequest {
        request_id: request_id.clone()
    })?;

    let event_name = format!("sudo-password-response-{}", request_id);
    let handler = app.listen(event_name, move |event| {
        let tx = tx.clone();
        tauri::async_runtime::spawn(async move {
            let password = serde_json::from_str(event.payload())
                .unwrap_or_default();

            let mut guard = tx.lock().await;
            if let Some(sender) = guard.take() {
                let _ = sender.send(password);
            }
        });
    });

    let password = tokio::time::timeout(
        std::time::Duration::from_secs(120),
        rx
    ).await??;

    app.unlisten(handler);
    Ok(password)
}

#[tauri::command]
pub async fn install_mysql(app: AppHandle) -> Result<(), String> {
    let password = get_sudo_password(&app).await.map_err(|e| e.to_string())?;

    // Combine all commands into a single shell script
    let combined_command = "apt update && apt install -y mysql-server";
    let full_cmd = format!("echo {} | sudo -S bash -c '{}'", password, combined_command);
    
    println!("Executing command: echo ****** | sudo -S bash -c '{}'", combined_command);

    let (mut rx, _child) = app.shell()
        .command("bash")
        .args(["-c", &full_cmd])
        .spawn()
        .map_err(|e| e.to_string())?;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                let log_line = format!("[MySQL Installation] {}", String::from_utf8_lossy(&line));
                println!("BACKEND LOG: {}", log_line);
                app.emit("mysql-install-log", log_line).unwrap();
            }
            CommandEvent::Stderr(line) => {
                let err_line = format!("[MySQL Installation Error] {}", String::from_utf8_lossy(&line));
                println!("BACKEND ERROR: {}", err_line);
                app.emit("mysql-install-error", err_line).unwrap();
            }
            CommandEvent::Terminated(status) => {
                match status.code {
                    Some(0) => (), // Success
                    Some(code) => return Err(format!("Command failed with exit code {}: MySQL installation", code)),
                    None => return Err("Command was terminated by a signal during MySQL installation".into()),
                }
            }
            _ => {}
        }
    }

    Ok(())
}

#[tauri::command]
pub fn is_mysql_installed() -> bool {
    use std::process::Command;
    
    Command::new("mysql")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}