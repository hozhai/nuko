use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    process::{ChildStdin, Command, Stdio},
    sync::{Mutex, OnceLock},
    thread,
};

use crate::{
    download::download_server_jar,
    filesystem::{self, create_eula_txt, create_nuko_properties},
    models::{Instance, InstanceConfig, InstanceInfo, InstanceMetrics},
};
use tauri::{Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

fn get_logs_map() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static LOGS: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    LOGS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_stdin_map() -> &'static Mutex<HashMap<String, ChildStdin>> {
    static STDIN: OnceLock<Mutex<HashMap<String, ChildStdin>>> = OnceLock::new();
    STDIN.get_or_init(|| Mutex::new(HashMap::new()))
}

fn get_system() -> &'static Mutex<sysinfo::System> {
    static SYS: OnceLock<Mutex<sysinfo::System>> = OnceLock::new();
    SYS.get_or_init(|| Mutex::new(sysinfo::System::new()))
}

#[tauri::command]
pub async fn get_instance_logs(id: String) -> Result<Vec<String>, String> {
    let logs_map = get_logs_map().lock().unwrap();
    Ok(logs_map.get(&id).cloned().unwrap_or_default())
}

/// Create a new Minecraft server instance with the given name, software, version, and optional loader
#[tauri::command]
pub async fn create_instance(
    app_handle: tauri::AppHandle,
    name: String,
    software: String,
    version: String,
    loader: Option<String>,
    icon_path: Option<String>,
    custom_jar_path: Option<String>,
) -> Result<(), String> {
    let server = Instance {
        name,
        software,
        version,
        loader,
        custom_jar_path,
    };

    let data_dir = filesystem::get_data_dir(&app_handle)?;

    if data_dir.join("instances").join(&server.name).exists() {
        return Err(format!("Instance '{}' already exists", server.name));
    }

    let instance_dir = filesystem::create_directory(data_dir, &server.name)
        .await
        .map_err(|e| format!("Error calling create_directory: {}", e))?;

    if let Some(icon) = icon_path {
        std::fs::copy(&icon, instance_dir.join("server-icon.png"))
            .map_err(|e| format!("Failed to copy server icon: {}", e))?;
    }

    create_nuko_properties(&instance_dir, &server)
        .await
        .map_err(|e| format!("Error calling create_nuko_manifest: {}", e))?;

    download_server_jar(&instance_dir, server)
        .await
        .map_err(|e| format!("Error calling download_server_jar: {}", e))?;

    create_eula_txt(&instance_dir)
        .await
        .map_err(|e| format!("Error calling create_eula_txt: {}", e))?;

    let _ = app_handle.emit("instances-updated", ());

    Ok(())
}

/// Lists all existing instances by reading the data directory and returning the name
/// stored in nuko.toml of subdirectories in the instances folder, and whether they're
/// running or not
#[tauri::command]
pub async fn list_instances(app_handle: tauri::AppHandle) -> Result<Vec<InstanceInfo>, String> {
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instances_dir = data_dir.join("instances");

    if !instances_dir.exists() {
        return Ok(vec![]);
    }

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut instances = Vec::new();

    for item in std::fs::read_dir(instances_dir)
        .map_err(|e| format!("Failed to read instances directory: {}", e))?
    {
        let entry = item.map_err(|e| format!("Failed to read instance entry: {}", e))?;
        if entry
            .file_type()
            .map_err(|e| format!("Failed to get file type: {}", e))?
            .is_dir()
        {
            let config_path = entry.path().join("nuko.toml");
            if config_path.exists() {
                let config_content = std::fs::read_to_string(&config_path)
                    .map_err(|e| format!("Failed to read nuko.toml: {}", e))?;
                let config: crate::models::InstanceConfig = toml::from_str(&config_content)
                    .map_err(|e| format!("Failed to parse nuko.toml: {}", e))?;

                let instance_path = entry.path();
                let mut running = false;
                for (_pid, process) in sys.processes() {
                    if let Some(cwd) = process.cwd() {
                        if cwd == instance_path {
                            running = true;
                            break;
                        }
                    }
                }

                instances.push(InstanceInfo {
                    id: config.id,
                    name: config.name,
                    software: config.software,
                    version: config.version,
                    running,
                });
            }
        }
    }

    Ok(instances)
}

#[tauri::command]
pub async fn get_instance_info(
    app_handle: tauri::AppHandle,
    id: String,
) -> Result<InstanceInfo, String> {
    let config = get_instance_by_id(&app_handle, &id).await;
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instance_dir = data_dir.join("instances").join(&config.name);

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut running = false;
    for (_pid, process) in sys.processes() {
        if let Some(cwd) = process.cwd() {
            if cwd == instance_dir {
                running = true;
                break;
            }
        }
    }

    Ok(InstanceInfo {
        id: config.id,
        name: config.name,
        software: config.software,
        version: config.version,
        running,
    })
}

#[tauri::command]
pub async fn get_instance_metrics(
    app_handle: tauri::AppHandle,
    id: String,
) -> Result<InstanceMetrics, String> {
    let config = get_instance_by_id(&app_handle, &id).await;
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instance_dir = data_dir.join("instances").join(&config.name);

    let mut sys = get_system().lock().unwrap();
    sys.refresh_processes(sysinfo::ProcessesToUpdate::All, true);

    let mut cpu_usage = 0.0;
    let mut memory_usage = 0;

    for (_pid, process) in sys.processes() {
        if let Some(cwd) = process.cwd() {
            if cwd == instance_dir {
                cpu_usage += process.cpu_usage();
                memory_usage += process.memory();
            }
        }
    }

    let time = chrono::Local::now().format("%H:%M:%S").to_string();

    Ok(InstanceMetrics {
        time,
        cpu_usage,
        memory_usage,
    })
}

#[tauri::command]
pub async fn stop_instance(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let instance = get_instance_by_id(&app_handle, &id).await;
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instance_dir = data_dir.join("instances").join(&instance.name);

    let mut sent_stop = false;
    {
        let mut stdin_map = get_stdin_map().lock().unwrap();
        if let Some(mut stdin) = stdin_map.remove(&id) {
            if writeln!(stdin, "stop").is_ok() && stdin.flush().is_ok() {
                sent_stop = true;
            }
        }
    }

    if !sent_stop {
        let mut sys = sysinfo::System::new_all();
        sys.refresh_all();

        let mut found = false;
        for (_pid, process) in sys.processes() {
            if let Some(cwd) = process.cwd() {
                if cwd == instance_dir {
                    let _ = process.kill_with(sysinfo::Signal::Term);
                    found = true;
                }
            }
        }

        if !found {
            return Err(format!("Instance '{}' is not running", instance.name));
        }
    }

    let _ = app_handle.emit("instances-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn kill_instance(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    {
        let mut stdin_map = get_stdin_map().lock().unwrap();
        stdin_map.remove(&id);
    }

    let instance = get_instance_by_id(&app_handle, &id).await;
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instance_dir = data_dir.join("instances").join(&instance.name);

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut found = false;
    for (_pid, process) in sys.processes() {
        if let Some(cwd) = process.cwd() {
            if cwd == instance_dir {
                let _ = process.kill_with(sysinfo::Signal::Kill);
                found = true;
            }
        }
    }

    if !found {
        return Err(format!("Instance '{}' is not running", instance.name));
    }

    let _ = app_handle.emit("instances-updated", ());
    Ok(())
}

#[tauri::command]
pub async fn restart_instance(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let _ = stop_instance(app_handle.clone(), id.clone()).await;

    let instance = get_instance_by_id(&app_handle, &id).await;
    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instance_dir = data_dir.join("instances").join(&instance.name);

    let mut sys = sysinfo::System::new_all();
    for _ in 0..60 {
        sys.refresh_all();
        let mut found = false;
        for (_pid, process) in sys.processes() {
            if let Some(cwd) = process.cwd() {
                if cwd == instance_dir {
                    found = true;
                    break;
                }
            }
        }
        if !found {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    start_instance(app_handle, id).await
}

#[tauri::command]
pub async fn open_instance_view(
    app_handle: tauri::AppHandle,
    id: String,
    name: String,
) -> Result<(), String> {
    if let Some(existing) = app_handle.get_webview_window(&format!("instance-{}", id)) {
        existing.set_focus().map_err(|e| e.to_string())?;
        return Ok(());
    }

    WebviewWindowBuilder::new(
        &app_handle,
        &format!("instance-{}", id),
        WebviewUrl::App(format!("/{}", id).into()),
    )
    .title(format!("nuko | {}", name))
    .inner_size(1000., 800.)
    .build()
    .map_err(|e| e.to_string())?;

    Ok(())
}

pub async fn get_instance_by_id(app_handle: &tauri::AppHandle, id: &String) -> InstanceConfig {
    let data_dir = filesystem::get_data_dir(app_handle).unwrap();
    let instances_dir = data_dir.join("instances");

    for item in std::fs::read_dir(instances_dir).unwrap() {
        let entry = item.unwrap();
        if entry.file_type().unwrap().is_dir() {
            let config_path = entry.path().join("nuko.toml");
            if config_path.exists() {
                let config_content = std::fs::read_to_string(&config_path).unwrap();
                let config: crate::models::InstanceConfig =
                    toml::from_str(&config_content).unwrap();

                if config.id == *id {
                    return config;
                }
            }
        }
    }

    panic!("Instance with id {} not found", id);
}

#[tauri::command]
pub async fn start_instance(app_handle: tauri::AppHandle, id: String) -> Result<(), String> {
    let instance = get_instance_by_id(&app_handle, &id).await;

    let data_dir = filesystem::get_data_dir(&app_handle)?;
    let instance_dir = data_dir.join("instances").join(&instance.name);

    if !instance_dir.exists() {
        return Err(format!("Instance '{}' does not exist", instance.name));
    }

    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();
    for (_pid, process) in sys.processes() {
        if let Some(cwd) = process.cwd() {
            if cwd == instance_dir {
                return Err(format!("Instance '{}' is already running", instance.name));
            }
        }
    }

    let java_path = instance
        .java
        .java_path
        .unwrap_or_else(|| "java".to_string());

    let mut cmd = Command::new(java_path);
    cmd.current_dir(&instance_dir);

    if !instance.java.min_memory.is_empty() {
        cmd.arg(format!("-Xms{}", instance.java.min_memory));
    }
    if !instance.java.max_memory.is_empty() {
        cmd.arg(format!("-Xmx{}", instance.java.max_memory));
    }

    for arg in instance.java.additional_args {
        cmd.arg(arg);
    }

    cmd.arg("-jar").arg("server.jar").arg("nogui");

    let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to start Java process: {}", e))?;

    if let Some(stdin) = child.stdin.take() {
        let mut stdin_map = get_stdin_map().lock().unwrap();
        stdin_map.insert(id.clone(), stdin);
    }

    {
        let mut logs_map = get_logs_map().lock().unwrap();
        logs_map.insert(id.clone(), Vec::new());
    }

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    let app_clone = app_handle.clone();
    let id_clone = id.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(line) = line {
                {
                    let mut logs_map = get_logs_map().lock().unwrap();
                    if let Some(logs) = logs_map.get_mut(&id_clone) {
                        logs.push(line.clone());
                    }
                }
                let _ = app_clone.emit(&format!("instance-log-{}", id_clone), line);
            }
        }
    });

    let app_clone_err = app_handle.clone();
    let id_clone_err = id.clone();
    thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines() {
            if let Ok(line) = line {
                {
                    let mut logs_map = get_logs_map().lock().unwrap();
                    if let Some(logs) = logs_map.get_mut(&id_clone_err) {
                        logs.push(line.clone());
                    }
                }
                let _ = app_clone_err.emit(&format!("instance-log-{}", id_clone_err), line);
            }
        }
    });

    let app_clone_wait = app_handle.clone();
    let id_clone_wait = id.clone();
    thread::spawn(move || {
        let _ = child.wait();
        {
            let mut stdin_map = get_stdin_map().lock().unwrap();
            stdin_map.remove(&id_clone_wait);
        }
        let _ = app_clone_wait.emit("instances-updated", ());
    });

    let _ = app_handle.emit("instances-updated", ());

    Ok(())
}
