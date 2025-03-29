// src/mongodb_installer.rs

use tauri::AppHandle;
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
pub async fn install_mongodb(app: AppHandle) -> Result<(), String> {
    let password = get_sudo_password(&app).await.map_err(|e| e.to_string())?;

    // MongoDB 8.0 installation commands for Ubuntu 24.04 (Noble) - without system-wide upgrade
    let commands = [
        // Step 1: Update package database only (no upgrade)
        "apt-get update",
        
        // Step 2: Install required dependencies
        "apt-get install -y gnupg curl",
        
        // Step 3: Import MongoDB public GPG key (force overwrite)
        "curl -fsSL https://www.mongodb.org/static/pgp/server-8.0.asc | gpg --yes -o /usr/share/keyrings/mongodb-server-8.0.gpg --dearmor",
        
        // Step 4: Add MongoDB repository
        "echo \"deb [ arch=amd64,arm64 signed-by=/usr/share/keyrings/mongodb-server-8.0.gpg ] https://repo.mongodb.org/apt/ubuntu noble/mongodb-org/8.0 multiverse\" | tee /etc/apt/sources.list.d/mongodb-org-8.0.list",
        
        // Step 5: Reload package database for MongoDB repo only
        "apt-get update -o Dir::Etc::sourcelist=\"sources.list.d/mongodb-org-8.0.list\" -o Dir::Etc::sourceparts=\"-\" -o APT::Get::List-Cleanup=\"0\"",
        
        // Step 6: Install MongoDB packages only
        "DEBIAN_FRONTEND=noninteractive apt-get install -y mongodb-org",
        
        // Step 7: Enable and start MongoDB service
        "systemctl daemon-reload && systemctl enable mongod && systemctl start mongod"
    ];
    
    // Execute each command separately to better identify failures
    for (i, cmd) in commands.iter().enumerate() {
        let step_num = i + 1;
        let cmd_desc = match i {
            0 => "Updating package database",
            1 => "Installing dependencies",
            2 => "Importing MongoDB GPG key",
            3 => "Adding MongoDB repository",
            4 => "Updating MongoDB package database",
            5 => "Installing MongoDB packages",
            6 => "Starting MongoDB service",
            _ => "Unknown step"
        };
        
        app.emit("mongodb-install-log", format!("[Step {}/{}] {} - Starting", step_num, commands.len(), cmd_desc)).unwrap();
        
        let full_cmd = format!("echo {} | sudo -S bash -c '{}' 2>&1", password, cmd);
        
        let (mut rx, _child) = app.shell()
            .command("bash")
            .args(["-c", &full_cmd])
            .spawn()
            .map_err(|e| format!("Failed to spawn command at step {}: {}", step_num, e))?;

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    let log_line = format!("[Step {}/{}] {}", step_num, commands.len(), String::from_utf8_lossy(&line));
                    println!("BACKEND LOG: {}", log_line);
                    app.emit("mongodb-install-log", log_line).unwrap();
                }
                CommandEvent::Stderr(line) => {
                    let err_line = format!("[Step {}/{}] ERROR: {}", step_num, commands.len(), String::from_utf8_lossy(&line));
                    println!("BACKEND ERROR: {}", err_line);
                    app.emit("mongodb-install-error", err_line).unwrap();
                }
                CommandEvent::Terminated(status) => {
                    match status.code {
                        Some(0) => {
                            app.emit("mongodb-install-log", format!("[Step {}/{}] {} - Completed", step_num, commands.len(), cmd_desc)).unwrap();
                        },
                        Some(code) => {
                            let error_msg = format!("Command failed with exit code {} during step {}: {}", code, step_num, cmd_desc);
                            app.emit("mongodb-install-error", error_msg.clone()).unwrap();
                            return Err(error_msg);
                        },
                        None => {
                            let error_msg = format!("Command was terminated by a signal during step {}: {}", step_num, cmd_desc);
                            app.emit("mongodb-install-error", error_msg.clone()).unwrap();
                            return Err(error_msg);
                        },
                    }
                }
                _ => {}
            }
        }
    }

    app.emit("mongodb-install-log", "MongoDB 8.0 installation completed successfully").unwrap();
    Ok(())
}

#[tauri::command]
pub async fn is_mongodb_installed() -> bool {
    use std::process::Command;
    
    println!("Checking MongoDB installation status...");
    
    // Check if mongod service exists
    let service_check = Command::new("systemctl")
        .args(["list-unit-files", "mongod.service"])
        .output()
        .map(|output| {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let contains_service = output_str.contains("mongod.service");
            println!("Service check output: {}", output_str);
            println!("Service check result: {}", contains_service);
            contains_service
        })
        .unwrap_or_else(|e| {
            println!("Service check error: {}", e);
            false
        });
    
    // Check if the mongod binary is installed
    let binary_check = Command::new("which")
        .arg("mongod")
        .output()
        .map(|output| {
            let success = output.status.success();
            let path = String::from_utf8_lossy(&output.stdout);
            println!("Binary check result: {} (path: {})", success, path);
            success
        })
        .unwrap_or_else(|e| {
            println!("Binary check error: {}", e);
            false
        });
    
    // Additional check: try to get MongoDB version
    let version_check = Command::new("sh")
        .args(["-c", "mongod --version | head -n1"])
        .output()
        .map(|output| {
            let version_str = String::from_utf8_lossy(&output.stdout);
            println!("Version check output: {}", version_str);
            output.status.success() && !version_str.is_empty()
        })
        .unwrap_or_else(|e| {
            println!("Version check error: {}", e);
            false
        });
    
    println!("MongoDB checks: service={}, binary={}, version={}", service_check, binary_check, version_check);
    
    // Return true if at least two of three checks pass
    let check_count = [service_check, binary_check, version_check].iter().filter(|&&check| check).count();
    let result = check_count >= 2;
    
    println!("Final MongoDB installation status: {}", result);
    result
}