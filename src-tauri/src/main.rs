#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::process::Command;
use std::sync::Arc;
use tauri::{
    CustomMenuItem, Manager, SystemTray, SystemTrayEvent, SystemTrayMenu,
    SystemTrayMenuItem,
};
use tokio::sync::Mutex;

struct AppState {
    ollama_process: Arc<Mutex<Option<std::process::Child>>>,
    backend_process: Arc<Mutex<Option<std::process::Child>>>,
}

#[tauri::command]
async fn check_ollama_status() -> Result<bool, String> {
    let response = reqwest::get("http://localhost:11434/api/version")
        .await
        .map_err(|e| e.to_string())?;
    Ok(response.status().is_success())
}

#[tauri::command]
async fn check_backend_status() -> Result<bool, String> {
    let response = reqwest::get("http://localhost:8080/api/health")
        .await
        .map_err(|e| e.to_string())?;
    Ok(response.status().is_success())
}

fn get_ollama_path() -> String {
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    
    match (os, arch) {
        ("macos", "aarch64") => "binaries/ollama-aarch64-apple-darwin",
        ("macos", "x86_64") => "binaries/ollama-x86_64-apple-darwin",
        ("linux", "aarch64") => "binaries/ollama-aarch64-unknown-linux",
        ("linux", "x86_64") => "binaries/ollama-x86_64-unknown-linux",
        ("windows", _) => "binaries/ollama.exe",
        _ => panic!("Unsupported platform"),
    }.to_string()
}

async fn start_ollama() -> Result<std::process::Child, String> {
    let binary_path = get_ollama_path();
    println!("Starting Ollama from path: {}", binary_path);
    
    let process = Command::new(binary_path)
        .spawn()
        .map_err(|e| {
            eprintln!("Failed to start Ollama: {}", e);
            e.to_string()
        })?;
    
    Ok(process)
}

async fn start_backend() -> Result<std::process::Child, String> {
    println!("Starting Python backend...");
    
    let process = Command::new("python3")
        .args(&["-m", "backend.app"])
        .env("OLLAMA_BASE_URL", "http://localhost:11434")
        .current_dir("../")  // Move up one directory to find the backend module
        .spawn()
        .map_err(|e| {
            eprintln!("Failed to start backend: {}", e);
            e.to_string()
        })?;
    
    Ok(process)
}

fn main() {
    let tray_menu = SystemTrayMenu::new()
        .add_item(CustomMenuItem::new("open".to_string(), "Open"))
        .add_native_item(SystemTrayMenuItem::Separator)
        .add_item(CustomMenuItem::new("quit".to_string(), "Quit"));

    let system_tray = SystemTray::new().with_menu(tray_menu);

    tauri::Builder::default()
        .system_tray(system_tray)
        .on_system_tray_event(|app, event| match event {
            SystemTrayEvent::MenuItemClick { id, .. } => match id.as_str() {
                "quit" => {
                    app.exit(0);
                }
                "open" => {
                    let window = app.get_window("main").unwrap();
                    window.show().unwrap();
                }
                _ => {}
            },
            _ => {}
        })
        .manage(AppState {
            ollama_process: Arc::new(Mutex::new(None)),
            backend_process: Arc::new(Mutex::new(None)),
        })
        .setup(|app| {
            let app_handle = app.handle();
            
            tauri::async_runtime::spawn(async move {
                // Start Ollama in the background
                match start_ollama().await {
                    Ok(ollama_process) => {
                        let state: tauri::State<AppState> = app_handle.state();
                        *state.ollama_process.lock().await = Some(ollama_process);
                        
                        // Wait for Ollama to start
                        for _ in 0..30 {
                            if check_ollama_status().await.unwrap_or(false) {
                                break;
                            }
                            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                        }
                        
                        // Start the Python backend
                        match start_backend().await {
                            Ok(backend_process) => {
                                *state.backend_process.lock().await = Some(backend_process);
                                
                                // Wait for backend to start
                                for _ in 0..30 {
                                    if check_backend_status().await.unwrap_or(false) {
                                        break;
                                    }
                                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                                }
                                
                                // Show the window once both services are ready
                                if let Some(window) = app_handle.get_window("main") {
                                    window.show().unwrap();
                                }
                            }
                            Err(e) => {
                                eprintln!("Failed to start backend: {}", e);
                                app_handle.exit(1);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to start Ollama: {}", e);
                        app_handle.exit(1);
                    }
                }
            });
            
            Ok(())
        })
        .on_window_event(|event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
                event.window().hide().unwrap();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
