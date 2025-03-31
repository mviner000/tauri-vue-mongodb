// src/mongodb_installer/windows.rs

use tauri::AppHandle;
use tauri::Emitter;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;
use anyhow::Result;
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::fs;
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone)]
pub struct InstallProgress {
    step: usize,
    total_steps: usize,
    message: String,
    is_error: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct DownloadProgress {
    bytes_downloaded: u64,
    total_bytes: u64,
    percentage: f64,
}

pub async fn install_mongodb(app: &AppHandle) -> Result<(), String> {
    // Define the MongoDB Windows download and installation parameters
    let mongodb_version = "8.0.6";
    let download_url = format!("https://fastdl.mongodb.org/windows/mongodb-windows-x86_64-{}-signed.msi", mongodb_version);
    let installer_filename = format!("mongodb-installer-{}.msi", Uuid::new_v4());
    let installer_path = std::env::temp_dir().join(installer_filename);
    let data_dir = r"C:\data\db";
    let mongo_bin_path = format!(r"C:\Program Files\MongoDB\Server\{}\bin", mongodb_version);

    // Define the steps for MongoDB installation
    let total_steps = 5;
    
    // Emit the installer path to the frontend
    if let Some(path_str) = installer_path.to_str() {
        app.emit("mongodb-installer-path", path_str.to_string()).unwrap_or_default();
    }
    
    // Step 1: Create data directory
    emit_progress(app, 1, total_steps, "Creating MongoDB data directory", false);
    create_directory(&data_dir)
        .map_err(|e| format!("Failed to create data directory: {}", e))?;
    
    // Step 2: Download MongoDB MSI installer
    emit_progress(app, 2, total_steps, "Downloading MongoDB installer", false);
    
    let installer_str = installer_path.to_str().unwrap();
    download_file_with_progress(app, &download_url, installer_str)
        .await
        .map_err(|e| format!("Failed to download MongoDB installer: {}", e))?;
    
    // Rest of the function remains unchanged...
    // Step 3: Install MongoDB silently
    emit_progress(app, 3, total_steps, "Installing MongoDB", false);
    install_mongodb_msi(app, installer_str)
        .await
        .map_err(|e| format!("Failed to install MongoDB: {}", e))?;
    
    // Step 4: Add MongoDB to PATH
    emit_progress(app, 4, total_steps, "Adding MongoDB to system PATH", false);
    add_to_path(app, &mongo_bin_path)
        .await
        .map_err(|e| format!("Failed to add MongoDB to PATH: {}", e))?;
    
    // Step 5: Start MongoDB service
    emit_progress(app, 5, total_steps, "Starting MongoDB service", false);
    start_mongodb_service(app, &mongo_bin_path, &data_dir)
        .await
        .map_err(|e| format!("Failed to start MongoDB service: {}", e))?;

    emit_progress(app, total_steps, total_steps, "MongoDB installation completed successfully", false);
    Ok(())
}

fn emit_progress(app: &AppHandle, step: usize, total_steps: usize, message: &str, is_error: bool) {
    let progress = InstallProgress {
        step,
        total_steps,
        message: message.to_string(),
        is_error,
    };
    
    if is_error {
        app.emit("mongodb-install-error", progress).unwrap_or_default();
    } else {
        app.emit("mongodb-install-log", progress).unwrap_or_default();
    }
    
    println!("[{}/{}] {}", step, total_steps, message);
}

fn create_directory(dir: &str) -> Result<(), std::io::Error> {
    fs::create_dir_all(dir)
}

async fn download_file_with_progress(app: &AppHandle, url: &str, out_path: &str) -> Result<(), String> {
    let (mut rx_head, _child_head) = app.shell()
        .command("powershell")
        .args(["-Command", &format!(
            "$ProgressPreference = 'SilentlyContinue'; 
             try {{ 
                $response = Invoke-WebRequest -Uri '{}' -Method Head -UseBasicParsing;
                $response.Headers.'Content-Length'
             }} catch {{ 
                Write-Host \"Error getting file size: $_.Exception.Message\";
                '0' 
             }}",
            url
        )])
        .spawn()
        .map_err(|e| format!("Failed to retrieve file size: {}", e))?;

    let mut total_bytes: u64 = 0;
    
    while let Some(event) = rx_head.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                let size_str = String::from_utf8_lossy(&line).trim().to_string();
                if size_str.starts_with("Error") {
                    println!("{}", size_str);
                } else if let Ok(size) = size_str.parse::<u64>() {
                    total_bytes = size;
                    println!("Total file size: {} bytes", total_bytes);
                    
                    let initial_progress = DownloadProgress {
                        bytes_downloaded: 0,
                        total_bytes,
                        percentage: 0.0,
                    };
                    app.emit("mongodb-download-progress", initial_progress).unwrap_or_default();
                }
            }
            CommandEvent::Terminated(status) => {
                if status.code.unwrap_or(-1) != 0 {
                    println!("Warning: File size check terminated with code: {:?}", status.code);
                }
            }
            _ => {}
        }
    }
    
    if total_bytes == 0 {
        total_bytes = 500 * 1024 * 1024;
        println!("Couldn't determine file size, using estimate: {} bytes", total_bytes);
    }

    let temp_dir = std::env::temp_dir();
    let script_name = format!("download_mongodb_{}.ps1", Uuid::new_v4());
    let ps_script_path = temp_dir.join(script_name);

    let ps_script_content = format!(r#"
        $url = "{}"
        $outPath = '{}'
        $tempOutPath = "$outPath.tmp"
        $totalBytes = {}
        
        function Write-ProgressToHost {{
            param (
                [long]$BytesReceived,
                [long]$TotalBytes,
                [double]$Percentage
            )
            $progressData = @{{
                "bytesDownloaded" = $BytesReceived
                "totalBytes" = $TotalBytes
                "percentage" = $Percentage
            }} | ConvertTo-Json -Compress
            
            Write-Host "PROGRESS: $progressData"
            [Console]::Out.Flush()
        }}
        
        $retryCount = 0
        $maxRetries = 5
        $downloadSuccess = $false
        
        while (-not $downloadSuccess -and $retryCount -lt $maxRetries) {{
            $retryCount++
            Write-Host "Attempting download (try $retryCount of $maxRetries)"
            
            try {{
                Write-Host "METHOD: Using Invoke-WebRequest download method"
                
                $webClient = New-Object System.Net.WebClient
                $webClient.Headers.Add("User-Agent", "Mozilla/5.0")
                
                Register-ObjectEvent -InputObject $webClient -EventName DownloadProgressChanged -Action {{
                    $bytesReceived = $EventArgs.BytesReceived
                    $percentage = [math]::Round(($bytesReceived / $totalBytes) * 100, 2)
                    Write-ProgressToHost $bytesReceived $totalBytes $percentage
                }}
                
                Register-ObjectEvent -InputObject $webClient -EventName DownloadFileCompleted -Action {{
                    if ($EventArgs.Error) {{
                        Write-Host "Download completed with error: $($EventArgs.Error.Message)"
                    }} else {{
                        Write-Host "COMPLETE: Download finished successfully"
                    }}
                }}
                
                $webClient.DownloadFileAsync([Uri]$url, $tempOutPath)
                
                while ($webClient.IsBusy) {{
                    Start-Sleep -Milliseconds 200
                }}
                
                if (Test-Path $tempOutPath) {{
                    $fileInfo = Get-Item $tempOutPath
                    
                    if ($fileInfo.Length -gt 0) {{
                        try {{
                            Move-Item -Path $tempOutPath -Destination $outPath -Force
                            $downloadSuccess = $true
                            Write-Host "Download succeeded, file moved to final location"
                        }} catch {{
                            Copy-Item -Path $tempOutPath -Destination $outPath -Force
                            Remove-Item -Path $tempOutPath -Force -ErrorAction SilentlyContinue
                            
                            if (Test-Path $outPath) {{
                                $downloadSuccess = $true
                                Write-Host "Download succeeded, file copied to final location"
                            }}
                        }}
                    }} else {{
                        Write-Host "Downloaded file has zero length"
                    }}
                }} else {{
                    Write-Host "Download failed, no file found"
                }}
                
                if (-not $downloadSuccess) {{
                    Write-Host "Trying alternative download method..."
                    
                    $client = New-Object System.Net.WebClient
                    $client.Headers.Add("User-Agent", "Mozilla/5.0")
                    $client.DownloadFile($url, $tempOutPath)
                    
                    if (Test-Path $tempOutPath) {{
                        $fileInfo = Get-Item $tempOutPath
                        if ($fileInfo.Length -gt 0) {{
                            Move-Item -Path $tempOutPath -Destination $outPath -Force
                            $downloadSuccess = $true
                            Write-Host "Alternative download succeeded"
                        }}
                    }}
                }}
                
                if (Test-Path $outPath) {{
                    $fileInfo = Get-Item $outPath
                    Write-Host "Final file size: $($fileInfo.Length) bytes"
                    
                    if ($fileInfo.Length -gt 0) {{
                        $downloadSuccess = $true
                    }} else {{
                        throw "Downloaded file has zero length"
                    }}
                }}
                
            }} catch {{
                Write-Host "Download attempt $retryCount failed: $($_.Exception.Message)"
                
                if ($retryCount -ge $maxRetries) {{
                    Write-Error "All $maxRetries download attempts failed. Last error: $($_.Exception.Message)"
                    exit 1
                }}
                
                $backoffTime = [math]::Min(30, [math]::Pow(2, $retryCount))
                Write-Host "Waiting $backoffTime seconds before retry..."
                Start-Sleep -Seconds $backoffTime
            }}
        }}
        
        if (-not $downloadSuccess) {{
            Write-Error "All download attempts failed after $maxRetries retries."
            exit 1
        }}
        
        if (-not (Test-Path $outPath)) {{
            Write-Error "Critical failure: Download reported success but file doesn't exist."
            exit 1
        }}
        
        $fileInfo = Get-Item $outPath
        Write-Host "Final file size: $($fileInfo.Length) bytes"
        
        exit 0
    "#, url, out_path.replace('\\', "\\\\"), total_bytes);

    fs::write(&ps_script_path, ps_script_content).map_err(|e| format!("Failed to create download script: {}", e))?;
    
    let (mut rx, _child) = app.shell()
        .command("powershell")
        .args(["-ExecutionPolicy", "Bypass", "-File", ps_script_path.to_str().unwrap()])
        .spawn()
        .map_err(|e| format!("Failed to spawn download script: {}", e))?;

    let mut last_progress_percentage = 0.0;
    
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                let line_str = String::from_utf8_lossy(&line);
                println!("Script output: {}", line_str.trim());
                
                if line_str.contains("PROGRESS:") {
                    let json_str = line_str.replace("PROGRESS:", "").trim().to_string();
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json_str) {
                        if let (Some(bytes), Some(total), Some(percentage)) = (
                            parsed["bytesDownloaded"].as_u64(),
                            parsed["totalBytes"].as_u64(),
                            parsed["percentage"].as_f64()
                        ) {
                            let progress = DownloadProgress {
                                bytes_downloaded: bytes,
                                total_bytes: total, 
                                percentage,
                            };
                            
                            app.emit("mongodb-download-progress", progress.clone()).unwrap_or_default();
                        }
                    }
                } else if line_str.contains("COMPLETE:") {
                    println!("Download completed");
                    if !Path::new(out_path).exists() {
                        return Err("Download marked complete but file missing".into());
                    }
                    app.emit("mongodb-download-progress", DownloadProgress {
                        bytes_downloaded: total_bytes,
                        total_bytes,
                        percentage: 100.0,
                    }).unwrap_or_default();
                    
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                } else if line_str.contains("METHOD:") {
                    let method_msg = format!("Download method: {}", line_str.replace("METHOD:", "").trim());
                    app.emit("mongodb-install-log", InstallProgress {
                        step: 2,
                        total_steps: 5,
                        message: method_msg,
                        is_error: false,
                    }).unwrap_or_default();
                }
            }
            CommandEvent::Stderr(line) => {
                let err_line = String::from_utf8_lossy(&line).trim().to_string();
                let err_msg = format!("Download error: {}", err_line);
                println!("{}", err_msg);
                
                app.emit("mongodb-install-error", InstallProgress {
                    step: 2,
                    total_steps: 5,
                    message: err_msg,
                    is_error: true,
                }).unwrap_or_default();
            }
            CommandEvent::Terminated(status) => {
                if status.code.unwrap_or(-1) != 0 {
                    return Err(format!("Download failed with exit code: {:?}", status.code));
                }
            }
            _ => {}
        }
    }

    let _ = fs::remove_file(&ps_script_path);

    if !Path::new(out_path).exists() {
        return Err("Download failed: output file does not exist".into());
    }

    Ok(())
}

async fn install_mongodb_msi(app: &AppHandle, installer_path: &str) -> Result<(), String> {
    // Step 1: Inform the user we're starting the manual installation
    emit_progress(
        app, 
        3, 
        5, 
        "Opening MongoDB installer. Please follow the on-screen instructions to complete the installation.", 
        false
    );

    // Step 2: Open the MSI file with the default program (Windows Installer)
    let (mut rx, _child) = app.shell()
        .command("powershell")
        .args([
            "-Command",
            &format!(
                "Start-Process '{}' -Wait",
                installer_path.replace('\\', "\\\\")
            )
        ])
        .spawn()
        .map_err(|e| format!("Failed to open the MongoDB installer: {}", e))?;

    // Step 3: Wait for the process to complete
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stderr(line) => {
                let err_line = String::from_utf8_lossy(&line).trim().to_string();
                if !err_line.is_empty() {
                    let err_msg = format!("Installation error: {}", err_line);
                    emit_progress(app, 3, 5, &err_msg, true);
                }
            }
            CommandEvent::Terminated(status) => {
                if status.code.unwrap_or(-1) != 0 {
                    return Err(format!("Installation process terminated with code: {:?}", status.code));
                }
                emit_progress(app, 3, 5, "MongoDB installation wizard completed", false);
            }
            _ => {}
        }
    }

    // Step 4: Verify installation
    emit_progress(app, 3, 5, "Verifying MongoDB installation...", false);
    
    // Give the installer a moment to finish
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    
    // Check for MongoDB installation path
    let (mut rx_verify, _child_verify) = app.shell()
        .command("powershell")
        .args([
            "-Command",
            "Test-Path 'C:\\Program Files\\MongoDB\\Server'"
        ])
        .spawn()
        .map_err(|e| format!("Failed to verify installation: {}", e))?;
    
    let mut is_installed = false;
    
    while let Some(event) = rx_verify.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                let output = String::from_utf8_lossy(&line).trim().to_string();
                if output.to_lowercase() == "true" {
                    is_installed = true;
                }
            }
            CommandEvent::Terminated(_) => {
                if !is_installed {
                    emit_progress(
                        app, 
                        3, 
                        5, 
                        "Warning: Could not verify MongoDB installation. If installation failed, please try again.",
                        true
                    );
                } else {
                    emit_progress(app, 3, 5, "MongoDB installation verified successfully", false);
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn add_to_path(app: &AppHandle, bin_path: &str) -> Result<(), String> {
    let (mut rx, _child) = app.shell()
        .command("powershell")
        .args([
            "-Command", 
            &format!(
                "$ErrorActionPreference = 'Stop';
                 try {{
                    $path = [Environment]::GetEnvironmentVariable('Path', 'Machine'); 
                    if (-not $path.Contains('{}')) {{ 
                       [Environment]::SetEnvironmentVariable('Path', \"$path;{}\", 'Machine');
                       Write-Output 'MongoDB bin directory added to PATH';
                    }} else {{
                       Write-Output 'MongoDB bin directory already in PATH';
                    }}
                 }} catch {{
                    Write-Error \"Failed to update PATH: $($_.Exception.Message)\";
                    exit 1;
                 }}", 
                bin_path.replace('\\', "\\\\"), 
                bin_path.replace('\\', "\\\\")
            )
        ])
        .spawn()
        .map_err(|e| format!("Failed to update PATH: {}", e))?;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                let output = String::from_utf8_lossy(&line).trim().to_string();
                println!("PATH update: {}", output);
                app.emit("mongodb-install-log", InstallProgress {
                    step: 4,
                    total_steps: 5,
                    message: output,
                    is_error: false,
                }).unwrap_or_default();
            }
            CommandEvent::Stderr(line) => {
                let err_line = String::from_utf8_lossy(&line).trim().to_string();
                let err_msg = format!("PATH update error: {}", err_line);
                println!("{}", err_msg);
                app.emit("mongodb-install-error", InstallProgress {
                    step: 4,
                    total_steps: 5,
                    message: err_msg,
                    is_error: true,
                }).unwrap_or_default();
            }
            CommandEvent::Terminated(status) => {
                if status.code.unwrap_or(-1) != 0 {
                    return Err(format!("PATH update failed with exit code: {:?}", status.code));
                }
            }
            _ => {}
        }
    }

    Ok(())
}

async fn start_mongodb_service(app: &AppHandle, bin_path: &str, data_dir: &str) -> Result<(), String> {
    // Try to start the MongoDB service first
    let (mut rx, _child) = app.shell()
        .command("powershell")
        .args([
            "-Command",
            "try { Start-Service -Name 'MongoDB' -ErrorAction Stop; 'Service started' } catch { 'Service not found' }"
        ])
        .spawn()
        .map_err(|e| format!("Failed to start MongoDB service: {}", e))?;

    let mut service_started = false;
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(line) => {
                let output = String::from_utf8_lossy(&line);
                if output.contains("Service started") {
                    service_started = true;
                }
            }
            CommandEvent::Terminated(status) => {
                if status.code.unwrap_or(-1) != 0 {
                    // Don't return error here as we'll try to start mongod manually
                    println!("Service start command failed with exit code: {:?}", status.code);
                }
            }
            _ => {}
        }
    }

    // If service wasn't started, try to run mongod directly
    if !service_started {
        emit_progress(app, 5, 5, "MongoDB service not found. Starting mongod manually...", false);
        
        let mongod_path = format!("{}\\mongod.exe", bin_path);
        let (mut rx, _child) = app.shell()
            .command("powershell")
            .args([
                "-Command",
                &format!(
                    "if (Test-Path '{}') {{ Start-Process '{}' -ArgumentList '--dbpath', '{}' -NoNewWindow -PassThru }}",
                    mongod_path.replace('\\', "\\\\"),
                    mongod_path.replace('\\', "\\\\"),
                    data_dir.replace('\\', "\\\\")
                )
            ])
            .spawn()
            .map_err(|e| format!("Failed to start mongod manually: {}", e))?;

        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stderr(line) => {
                    let err_line = format!("Mongod start error: {}", String::from_utf8_lossy(&line));
                    println!("{}", err_line);
                    app.emit("mongodb-install-error", InstallProgress {
                        step: 5,
                        total_steps: 5,
                        message: err_line,
                        is_error: true,
                    }).unwrap_or_default();
                }
                CommandEvent::Terminated(status) => {
                    if status.code.unwrap_or(-1) != 0 {
                        return Err(format!("Mongod start failed with exit code: {:?}", status.code));
                    }
                }
                _ => {}
            }
        }
    }

    Ok(())
}

pub async fn is_mongodb_installed() -> bool {
    use std::process::Command;
    
    println!("Checking MongoDB installation status on Windows...");
    
    // Check if MongoDB is installed as a service
    let service_check = Command::new("sc")
        .args(["query", "MongoDB"])
        .output()
        .map(|output| {
            let output_str = String::from_utf8_lossy(&output.stdout);
            let contains_service = !output_str.contains("DOES_NOT_EXIST");
            println!("Service check result: {}", contains_service);
            contains_service
        })
        .unwrap_or_else(|e| {
            println!("Service check error: {}", e);
            false
        });
    
    // Check if mongod.exe exists in the default installation path
    let path_exists = Path::new(r"C:\Program Files\MongoDB\Server").exists();
    println!("Path check result: {}", path_exists);
    
    // Try to connect to MongoDB
    let connection_check = Command::new("powershell")
        .args(["-Command", "try { New-Object System.Net.Sockets.TcpClient('localhost', 27017); $true } catch { $false }"])
        .output()
        .map(|output| {
            let output_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let can_connect = output_str == "True";
            println!("Connection check result: {}", can_connect);
            can_connect
        })
        .unwrap_or_else(|e| {
            println!("Connection check error: {}", e);
            false
        });
    
    // Return true if at least two of three checks pass
    let check_count = [service_check, path_exists, connection_check].iter().filter(|&&check| check).count();
    let result = check_count >= 2;
    
    println!("Final MongoDB installation status on Windows: {}", result);
    result
}