use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use tauri::{
    menu::{MenuBuilder, MenuItemBuilder},
    tray::TrayIconBuilder,
    Manager,
};

fn settings_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("C:\\Users\\NGOMI"));
    home.join(".claude").join("settings.json")
}

fn read_settings() -> Value {
    let path = settings_path();
    match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or(Value::Object(Default::default())),
        Err(_) => Value::Object(Default::default()),
    }
}

fn write_settings(data: &Value) {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let json = serde_json::to_string_pretty(data).unwrap_or_default();
    let _ = fs::write(&path, json);
}

fn extract_sound_path(settings: &Value) -> String {
    let default = "C:\\Windows\\Media\\notify.wav".to_string();
    let src = settings.get("hooks").or_else(|| settings.get("_hooksBackup"));
    let src = match src {
        Some(v) => v,
        None => return default,
    };
    if let Some(stop) = src.get("Stop") {
        if let Some(first) = stop.get(0) {
            if let Some(hooks) = first.get("hooks") {
                if let Some(hook) = hooks.get(0) {
                    if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                        if let Some(m) = extract_wav_path(cmd) {
                            return m;
                        }
                    }
                }
            }
        }
    }
    default
}

fn extract_ask_sound_path(settings: &Value) -> String {
    let default = "C:\\Windows\\Media\\Ring01.wav".to_string();
    let src = settings.get("hooks").or_else(|| settings.get("_hooksBackup"));
    let src = match src {
        Some(v) => v,
        None => return default,
    };
    if let Some(pre) = src.get("PreToolUse") {
        if let Some(first) = pre.get(0) {
            if let Some(hooks) = first.get("hooks") {
                if let Some(hook) = hooks.get(0) {
                    if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                        if let Some(m) = extract_wav_path(cmd) {
                            return m;
                        }
                    }
                }
            }
        }
    }
    default
}

fn extract_wav_path(cmd: &str) -> Option<String> {
    // Match single or double quoted .wav path
    let mut in_quote = false;
    let mut quote_char = ' ';
    let mut current = String::new();

    for ch in cmd.chars() {
        if !in_quote && (ch == '\'' || ch == '"') {
            in_quote = true;
            quote_char = ch;
            current.clear();
        } else if in_quote && ch == quote_char {
            in_quote = false;
            if current.to_lowercase().ends_with(".wav") {
                return Some(current);
            }
        } else if in_quote {
            current.push(ch);
        }
    }
    None
}

fn extract_ntfy_topic(settings: &Value) -> String {
    let src = settings.get("hooks").or_else(|| settings.get("_hooksBackup"));
    let src = match src {
        Some(v) => v,
        None => return String::new(),
    };
    if let Some(stop) = src.get("Stop") {
        if let Some(first) = stop.get(0) {
            if let Some(hooks) = first.get("hooks") {
                if let Some(arr) = hooks.as_array() {
                    for hook in arr {
                        if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                            if let Some(pos) = cmd.find("ntfy.sh/") {
                                let topic = &cmd[pos + 8..];
                                let topic = topic.split_whitespace().next().unwrap_or("");
                                if !topic.is_empty() {
                                    return topic.to_string();
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    String::new()
}

fn get_auto_start_enabled() -> bool {
    let output = Command::new("reg")
        .args([
            "query",
            r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
            "/v",
            "ClaudeNotify",
        ])
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).contains("ClaudeNotify"),
        Err(_) => false,
    }
}

fn set_auto_start(enabled: bool) {
    if enabled {
        let exe_path = std::env::current_exe()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let _ = Command::new("reg")
            .args([
                "add",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "ClaudeNotify",
                "/t",
                "REG_SZ",
                "/d",
                &exe_path,
                "/f",
            ])
            .output();
    } else {
        let _ = Command::new("reg")
            .args([
                "delete",
                r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run",
                "/v",
                "ClaudeNotify",
                "/f",
            ])
            .output();
    }
}

// ── Tauri Commands ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct Config {
    enabled: bool,
    sound_path: String,
    ask_sound_path: String,
    ntfy_topic: String,
    auto_start: bool,
}

#[derive(Deserialize)]
pub struct SaveConfigArgs {
    enabled: bool,
    sound_path: String,
    ask_sound_path: String,
    ntfy_topic: String,
    auto_start: bool,
}

#[tauri::command]
fn get_config() -> Config {
    let s = read_settings();
    Config {
        enabled: s.get("hooks").is_some(),
        sound_path: extract_sound_path(&s),
        ask_sound_path: extract_ask_sound_path(&s),
        ntfy_topic: extract_ntfy_topic(&s),
        auto_start: get_auto_start_enabled(),
    }
}

#[tauri::command]
fn save_config(args: SaveConfigArgs) -> Value {
    let mut s = read_settings();

    set_auto_start(args.auto_start);

    let stop_hooks = build_stop_hooks(&args.sound_path, &args.ntfy_topic);
    let pre_tool_use_hooks = build_pre_tool_use_hooks(&args.ask_sound_path, &args.ntfy_topic);
    let notification_hooks = build_notification_hooks(&args.ntfy_topic);

    if args.enabled {
        // Restore from backup if needed
        if s.get("_hooksBackup").is_some() && s.get("hooks").is_none() {
            if let Some(backup) = s.get("_hooksBackup").cloned() {
                s.as_object_mut().unwrap().insert("hooks".to_string(), backup);
            }
            s.as_object_mut().unwrap().remove("_hooksBackup");
        }
        if s.get("hooks").is_none() {
            s.as_object_mut()
                .unwrap()
                .insert("hooks".to_string(), Value::Object(Default::default()));
        }
        let hooks = s.get_mut("hooks").unwrap().as_object_mut().unwrap();
        hooks.insert("Stop".to_string(), stop_hooks);
        hooks.insert("PreToolUse".to_string(), pre_tool_use_hooks);
        if let Some(nh) = notification_hooks {
            hooks.insert("Notification".to_string(), nh);
        } else {
            hooks.remove("Notification");
        }
    } else {
        if s.get("hooks").is_some() {
            // Update hooks with latest values before backing up
            let hooks = s.get_mut("hooks").unwrap().as_object_mut().unwrap();
            hooks.insert("Stop".to_string(), stop_hooks);
            hooks.insert("PreToolUse".to_string(), pre_tool_use_hooks);
            if let Some(nh) = notification_hooks {
                hooks.insert("Notification".to_string(), nh);
            } else {
                hooks.remove("Notification");
            }
            // Move hooks to backup
            let hooks_val = s.get("hooks").cloned().unwrap();
            let obj = s.as_object_mut().unwrap();
            obj.insert("_hooksBackup".to_string(), hooks_val);
            obj.remove("hooks");
        }
    }

    write_settings(&s);
    serde_json::json!({ "ok": true })
}

fn build_stop_hooks(sound: &str, topic: &str) -> Value {
    let mut hooks = vec![serde_json::json!({
        "type": "command",
        "command": format!("powershell.exe -c \"(New-Object Media.SoundPlayer '{}').PlaySync()\"", sound)
    })];
    if !topic.is_empty() {
        hooks.push(serde_json::json!({
            "type": "command",
            "command": format!("curl -s -d \"Claude Code finished a task\" https://ntfy.sh/{}", topic)
        }));
    }
    serde_json::json!([{ "hooks": hooks }])
}

fn build_pre_tool_use_hooks(sound: &str, topic: &str) -> Value {
    let mut hooks = vec![serde_json::json!({
        "type": "command",
        "command": format!("powershell.exe -c \"(New-Object Media.SoundPlayer '{}').PlaySync()\"", sound)
    })];
    if !topic.is_empty() {
        hooks.push(serde_json::json!({
            "type": "command",
            "command": format!("curl -s -d \"Claude Code is asking you a question\" https://ntfy.sh/{}", topic)
        }));
    }
    serde_json::json!([{ "matcher": "AskUserQuestion", "hooks": hooks }])
}

fn build_notification_hooks(topic: &str) -> Option<Value> {
    if topic.is_empty() {
        return None;
    }
    Some(serde_json::json!([{
        "hooks": [{
            "type": "command",
            "command": format!("curl -s -d \"Claude Code needs your attention\" https://ntfy.sh/{}", topic)
        }]
    }]))
}

#[tauri::command]
fn test_sound(path: String) -> Value {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-c",
            &format!("(New-Object Media.SoundPlayer '{}').PlaySync()", path),
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => serde_json::json!({ "ok": true }),
        Ok(o) => serde_json::json!({
            "ok": false,
            "error": String::from_utf8_lossy(&o.stderr).to_string()
        }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

#[tauri::command]
fn test_ntfy(topic: String) -> Value {
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let output = Command::new("curl")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-s",
            "-d",
            "Test from Claude Notify app",
            &format!("https://ntfy.sh/{}", topic),
        ])
        .output();
    match output {
        Ok(o) if o.status.success() => serde_json::json!({ "ok": true }),
        Ok(o) => serde_json::json!({
            "ok": false,
            "error": String::from_utf8_lossy(&o.stderr).to_string()
        }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

// ── App Setup ──────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // Build tray menu
            let open_item = MenuItemBuilder::with_id("open", "Open Settings").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            let menu = MenuBuilder::new(app)
                .item(&open_item)
                .separator()
                .item(&quit_item)
                .build()?;

            // Build tray icon
            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Claude Code Notifications")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "open" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let tauri::tray::TrayIconEvent::DoubleClick {
                        button: tauri::tray::MouseButton::Left,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            if win.is_visible().unwrap_or(false) {
                                let _ = win.hide();
                            } else {
                                let _ = win.show();
                                let _ = win.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            // Hide window on close instead of exiting
            let win = app.get_webview_window("main").unwrap();
            let win_clone = win.clone();
            win.on_window_event(move |event| {
                if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = win_clone.hide();
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            test_sound,
            test_ntfy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
