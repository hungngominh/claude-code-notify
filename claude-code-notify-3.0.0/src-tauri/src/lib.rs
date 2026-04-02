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

const CREATE_NO_WINDOW: u32 = 0x08000000;

// ── FUTURE WORK: Happy Push Notifications ─────────────────────
// Happy integration is disabled in this version (v3.0.2).
// To re-enable: add `future_happy` to the default features in Cargo.toml.
// All Happy code is preserved under #[cfg(feature = "future_happy")].
// ─────────────────────────────────────────────────────────────

fn settings_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("settings.json")
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

#[cfg(feature = "future_happy")]
fn get_happy_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join("AppData")
        .join("Roaming")
        .join("npm")
        .join("happy.cmd")
}

// ── Saved config (reliable round-trip, no command parsing) ────

#[derive(Serialize, Deserialize, Clone, Default)]
struct SavedNotifyConfig {
    #[serde(default)]
    sound_path: String,
    #[serde(default)]
    ask_sound_path: String,
    #[serde(default)]
    gchat_webhook: String,
    #[serde(default)]
    toast_enabled: bool,
    #[cfg(feature = "future_happy")]
    #[serde(default)]
    happy_enabled: bool,
    #[cfg(feature = "future_happy")]
    #[serde(default)]
    happy_project_dir: String,
    #[cfg(feature = "future_happy")]
    #[serde(default)]
    happy_auto_daemon: bool,
    #[cfg(feature = "future_happy")]
    #[serde(default)]
    happy_projects: Vec<String>,
}

impl From<&SaveConfigArgs> for SavedNotifyConfig {
    fn from(a: &SaveConfigArgs) -> Self {
        #[cfg(feature = "future_happy")]
        {
            // Preserve existing happy_project_dir when saving config
            let existing_dir = {
                let s = read_settings();
                load_saved_config(&s)
                    .map(|c| c.happy_project_dir)
                    .unwrap_or_default()
            };
            return SavedNotifyConfig {
                sound_path: a.sound_path.clone(),
                ask_sound_path: a.ask_sound_path.clone(),
                gchat_webhook: a.gchat_webhook.clone(),
                toast_enabled: a.toast_enabled,
                happy_enabled: a.happy_enabled,
                happy_project_dir: existing_dir,
                happy_auto_daemon: a.happy_auto_daemon,
                happy_projects: a.happy_projects.clone(),
            };
        }
        #[cfg(not(feature = "future_happy"))]
        SavedNotifyConfig {
            sound_path: a.sound_path.clone(),
            ask_sound_path: a.ask_sound_path.clone(),
            gchat_webhook: a.gchat_webhook.clone(),
            toast_enabled: a.toast_enabled,
        }
    }
}

fn load_saved_config(settings: &Value) -> Option<SavedNotifyConfig> {
    settings
        .get("_claudeNotifyConfig")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn write_saved_config(settings: &mut Value, cfg: &SavedNotifyConfig) {
    if let Some(obj) = settings.as_object_mut() {
        if let Ok(v) = serde_json::to_value(cfg) {
            obj.insert("_claudeNotifyConfig".to_string(), v);
        }
    }
}

// ── Legacy extraction (fallback when _claudeNotifyConfig absent) ──

fn extract_wav_path(cmd: &str) -> Option<String> {
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

fn extract_sound_from_hooks(settings: &Value, hook_key: &str) -> Option<String> {
    let src = settings.get("hooks").or_else(|| settings.get("_hooksBackup"))?;
    let arr = src.get(hook_key)?.as_array()?;
    for entry in arr {
        // Use and_then instead of ? so a missing "hooks" field skips the entry
        if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
            for hook in hooks {
                if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                    if cmd.contains("SoundPlayer") {
                        if let Some(path) = extract_wav_path(cmd) {
                            return Some(path);
                        }
                    }
                }
            }
        }
    }
    None
}

fn extract_gchat_from_hooks(settings: &Value) -> Option<String> {
    let src = settings.get("hooks").or_else(|| settings.get("_hooksBackup"))?;
    let arr = src.get("Stop")?.as_array()?;
    for entry in arr {
        // Use and_then instead of ? so a missing "hooks" field skips the entry
        if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
            for hook in hooks {
                if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                    if cmd.contains("chat.googleapis.com") {
                        for part in cmd.split_whitespace() {
                            let trimmed = part.trim_matches('"').trim_matches('\'');
                            if trimmed.contains("chat.googleapis.com") && trimmed.starts_with("https://") {
                                return Some(trimmed.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

/// Generic: returns true if any hook command in the settings satisfies `predicate`.
fn detect_cmd_in_hooks<F: Fn(&str) -> bool>(settings: &Value, predicate: F) -> bool {
    let src = match settings.get("hooks").or_else(|| settings.get("_hooksBackup")) {
        Some(v) => v,
        None => return false,
    };
    if let Some(obj) = src.as_object() {
        for (_key, arr) in obj {
            if let Some(entries) = arr.as_array() {
                for entry in entries {
                    if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
                        for hook in hooks {
                            if let Some(cmd) = hook.get("command").and_then(|c| c.as_str()) {
                                if predicate(cmd) {
                                    return true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    false
}

// ── Hook fingerprinting (identify Claude Notify hooks vs others) ──

fn is_claude_notify_hook(cmd: &str) -> bool {
    cmd.contains("SoundPlayer")
        || cmd.contains("chat.googleapis.com")
        || cmd.contains("ToastNotificationManager")
        || cmd.contains("claude-notify-toast.ps1")
        || cmd.contains("claude-notify-toast.cjs")
        || cmd.contains("claude-notify-balloon.ps1")
        || cmd.contains("claude-notify-gchat.cjs")
        || cmd.contains("claude-notify-hook.cjs")
        || {
            #[cfg(feature = "future_happy")]
            { cmd.contains("claude-notify-happy.cjs") || (cmd.contains("happy") && cmd.contains("notify")) }
            #[cfg(not(feature = "future_happy"))]
            { false }
        }
}

/// Remove Claude Notify hook entries from a hook array, keeping all others.
fn filter_out_cn_entries(arr: &[Value]) -> Vec<Value> {
    let mut kept = Vec::new();
    for entry in arr {
        if let Some(hooks) = entry.get("hooks").and_then(|h| h.as_array()) {
            let non_cn: Vec<&Value> = hooks
                .iter()
                .filter(|h| {
                    h.get("command")
                        .and_then(|c| c.as_str())
                        .map(|cmd| !is_claude_notify_hook(cmd))
                        .unwrap_or(true)
                })
                .collect();
            if non_cn.is_empty() {
                continue;
            }
            let mut new_entry = entry.clone();
            if let Some(obj) = new_entry.as_object_mut() {
                obj.insert(
                    "hooks".to_string(),
                    Value::Array(non_cn.into_iter().cloned().collect()),
                );
            }
            kept.push(new_entry);
        } else {
            kept.push(entry.clone());
        }
    }
    kept
}

/// Merge Claude Notify's hook entry into an existing hook array.
fn merge_cn_entry(existing: &Value, cn_entry: Value) -> Value {
    let mut entries = match existing.as_array() {
        Some(arr) => filter_out_cn_entries(arr),
        None => Vec::new(),
    };
    entries.push(cn_entry);
    Value::Array(entries)
}

// ── Auto-start ────────────────────────────────────────────────

fn get_auto_start_enabled() -> bool {
    let output = Command::new("reg")
        .creation_flags(CREATE_NO_WINDOW)
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

fn get_installed_exe_path() -> Option<String> {
    // Installed location: %LOCALAPPDATA%\claude-notify-app\claude-notify-app.exe
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let installed = std::path::PathBuf::from(&local_app_data)
            .join("claude-notify-app")
            .join("claude-notify-app.exe");
        if installed.exists() {
            return Some(installed.to_string_lossy().to_string());
        }
    }
    None
}

fn set_auto_start(enabled: bool) {
    if enabled {
        // Always use the installed exe path, never the dev/debug path
        let exe_path = match get_installed_exe_path() {
            Some(p) => p,
            None => {
                // Fallback: only allow if current exe is NOT in a target/debug or target/release dir
                let current = std::env::current_exe().unwrap_or_default();
                let current_str = current.to_string_lossy().to_string();
                if current_str.contains("target\\debug") || current_str.contains("target\\release") || current_str.contains("target/debug") || current_str.contains("target/release") {
                    // Dev mode — don't register auto-start
                    return;
                }
                current_str
            }
        };
        let _ = Command::new("reg")
            .creation_flags(CREATE_NO_WINDOW)
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
            .creation_flags(CREATE_NO_WINDOW)
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

// ── Tauri Commands ────────────────────────────────────────────

#[derive(Serialize)]
pub struct Config {
    enabled: bool,
    sound_path: String,
    ask_sound_path: String,
    gchat_webhook: String,
    auto_start: bool,
    toast_enabled: bool,
    #[cfg(feature = "future_happy")]
    happy_enabled: bool,
    #[cfg(feature = "future_happy")]
    happy_auto_daemon: bool,
    #[cfg(feature = "future_happy")]
    happy_projects: Vec<String>,
}

#[derive(Deserialize)]
pub struct SaveConfigArgs {
    enabled: bool,
    sound_path: String,
    ask_sound_path: String,
    gchat_webhook: String,
    auto_start: bool,
    toast_enabled: bool,
    #[cfg(feature = "future_happy")]
    happy_enabled: bool,
    #[cfg(feature = "future_happy")]
    happy_auto_daemon: bool,
    #[cfg(feature = "future_happy")]
    happy_projects: Vec<String>,
}

// ── Hook command builders ─────────────────────────────────────


fn toast_command(title: &str, message: &str) -> Value {
    let script_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude");
    let _ = std::fs::create_dir_all(&script_dir);

    // Write toast PS1 script (also used by test_toast)
    let ps_path = script_dir.join("claude-notify-toast.ps1");
    let ps_content = r#"param([string]$Title, [string]$Message)
try {
    [Windows.UI.Notifications.ToastNotificationManager, Windows.UI.Notifications, ContentType = WindowsRuntime] | Out-Null
    [Windows.Data.Xml.Dom.XmlDocument, Windows.Data.Xml.Dom, ContentType = WindowsRuntime] | Out-Null

    $appId = '{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe'

    $template = @"
<toast>
  <visual>
    <binding template="ToastGeneric">
      <text>$([System.Security.SecurityElement]::Escape($Title))</text>
      <text>$([System.Security.SecurityElement]::Escape($Message))</text>
    </binding>
  </visual>
  <audio silent="true"/>
</toast>
"@

    $xml = New-Object Windows.Data.Xml.Dom.XmlDocument
    $xml.LoadXml($template)
    $toast = [Windows.UI.Notifications.ToastNotification]::new($xml)
    [Windows.UI.Notifications.ToastNotificationManager]::CreateToastNotifier($appId).Show($toast)
} catch {
    Write-Error $_.Exception.Message
    exit 1
}
"#;
    let _ = std::fs::write(&ps_path, ps_content);

    // Direct PowerShell call — no node wrapper, avoids process tree kill issue
    let ps_path_fwd = ps_path.to_string_lossy().replace('\\', "/");
    let cmd = format!(
        "powershell.exe -WindowStyle Hidden -ExecutionPolicy Bypass -File \"{}\" -Title \"{}\" -Message \"{}\"",
        ps_path_fwd, title, message
    );
    serde_json::json!({ "type": "command", "command": cmd })
}

fn gchat_card_json(title: &str, subtitle: &str, icon_url: &str, icon: &str) -> String {
    format!(
        r#"{{"cardsV2":[{{"cardId":"claude-notify","card":{{"header":{{"title":"{}","subtitle":"{}","imageUrl":"{}","imageType":"CIRCLE"}},"sections":[{{"widgets":[{{"decoratedText":{{"startIcon":{{"knownIcon":"{}"}},"text":"{}"}}}}]}}]}}}}]}}"#,
        title, subtitle, icon_url, icon, subtitle
    )
}

#[allow(dead_code)]
fn generate_gchat_wrapper() -> PathBuf {
    let script_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let _ = fs::create_dir_all(&script_dir);
    let wrapper_path = script_dir.join("claude-notify-gchat.cjs");
    let content = r#"'use strict';
let d='';
process.stdin.setEncoding('utf8');
process.stdin.on('data',c=>d+=c);
process.stdin.on('end',()=>{
  let project='',title='Claude Code',subtitle='',iconUrl='',icon='BOOKMARK';
  const ev=process.argv[2]||'stop';
  const webhook=process.argv[3]||'';
  if(!webhook){process.exit(0);}
  try{
    const j=JSON.parse(d);
    if(j.cwd) project=require('path').basename(j.cwd);
    if(ev==='stop'){
      title=project?`[${project}] Task Finished`:'Task Finished';
      subtitle=j.last_assistant_message?j.last_assistant_message.substring(0,300).replace(/[\r\n]+/g,' '):'Claude Code finished a task';
      iconUrl='https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/2705.png';
      icon='BOOKMARK';
    }else if(ev==='pre_tool_use'){
      title=project?`[${project}] Question`:'Question';
      let q='Claude Code is asking a question';
      if(j.tool_input){
        const qs=j.tool_input.questions;
        if(Array.isArray(qs)&&qs.length>0) q=qs[0].question||q;
        else if(j.tool_input.question) q=j.tool_input.question;
      }
      subtitle=q.substring(0,300).replace(/[\r\n]+/g,' ');
      iconUrl='https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/2753.png';
      icon='PERSON';
    }else if(ev==='notification'){
      title=project?`[${project}] Attention`:'Attention';
      subtitle=j.message||'Claude Code needs attention';
      iconUrl='https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/1f514.png';
      icon='DESCRIPTION';
    }else if(ev==='session_end'){
      title=project?`[${project}] Session Ended`:'Session Ended';
      subtitle='Claude Code session has ended';
      iconUrl='https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/1f6d1.png';
      icon='BOOKMARK';
    }else if(ev==='permission_request'){
      title=project?`[${project}] Permission`:'Permission';
      subtitle=j.tool_name?`Permission needed for ${j.tool_name}`:'Claude Code needs permission';
      iconUrl='https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/1f512.png';
      icon='PERSON';
    }
  }catch{}
  const card=JSON.stringify({cardsV2:[{cardId:'claude-notify',card:{
    header:{title,subtitle:subtitle.substring(0,200),imageUrl:iconUrl,imageType:'CIRCLE'},
    sections:[{widgets:[{decoratedText:{startIcon:{knownIcon:icon},text:subtitle.substring(0,300)}}]}]
  }}]});
  const escaped=card.replace(/'/g,"''");
  require('child_process').spawn(
    'powershell.exe',
    ['-WindowStyle','Hidden','-c',
     `try { Invoke-RestMethod -Uri '${webhook}' -Method POST -ContentType 'application/json' -Body '${escaped}' } catch {}; exit 0`],
    {stdio:'ignore',detached:true}).unref();
});
setTimeout(()=>process.exit(0),5000);
"#;
    let _ = fs::write(&wrapper_path, content);
    wrapper_path
}

#[cfg(feature = "future_happy")]
#[allow(dead_code)]
fn generate_happy_wrapper() -> PathBuf {
    let script_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let _ = fs::create_dir_all(&script_dir);
    let wrapper_path = script_dir.join("claude-notify-happy.cjs");
    let happy_path_fwd = get_happy_path().to_string_lossy().replace('\\', "/");
    let content = format!(
        r#"'use strict';
const fs=require('fs');
const cp=require('child_process');
const path=require('path');
let d='';
process.stdin.setEncoding('utf8');
process.stdin.on('data',c=>d+=c);
process.stdin.on('end',()=>{{
  let project='',body='',cwd='';
  const ev=process.argv[2]||'stop';
  const hp='{}';
  try{{
    const j=JSON.parse(d);
    cwd=j.cwd||'';
    if(cwd) project=path.basename(cwd);
    if(ev==='stop'){{
      body=j.last_assistant_message?j.last_assistant_message.substring(0,200):'Task completed';
    }}else if(ev==='pre_tool_use'){{
      if(j.tool_input){{
        const q=j.tool_input.questions;
        if(Array.isArray(q)&&q.length>0) body=q[0].question||'Needs your input';
        else body=j.tool_input.question||'Needs your input';
      }}else body='Needs your input';
    }}else if(ev==='notification'){{
      body=j.message||'Needs attention';
    }}else if(ev==='session_end'){{
      body='Session ended';
    }}else if(ev==='permission_request'){{
      body=j.tool_name?`Permission for ${{j.tool_name}}`:'Permission needed';
    }}
  }}catch{{}}
  const labels={{stop:'Done',pre_tool_use:'Question',notification:'Alert',session_end:'Ended',permission_request:'Permission'}};
  const title=project?`[${{project}}] ${{labels[ev]||'Claude Code'}}`:'Claude Code';
  const b=body||'Claude Code';
  const t=title.replace(/"/g,'');
  const m=b.replace(/"/g,'').substring(0,200);
  // Send push notification
  cp.exec(`"${{hp}}" notify -t "${{t}}" -p "${{m}}"`,{{timeout:10000}},()=>{{}});
}});
setTimeout(()=>process.exit(0),15000);
"#,
        happy_path_fwd
    );
    let _ = fs::write(&wrapper_path, content);
    wrapper_path
}

/// Generate a single combined hook script that reads stdin and handles all
/// notification channels (sound, toast, gchat). Called once in save_config.
#[allow(unused_variables)]
fn generate_hook_script(
    stop_sound: &str,
    ask_sound: &str,
    webhook: &str,
    toast: bool,
    #[cfg(feature = "future_happy")] happy: bool,
) {
    let script_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let _ = fs::create_dir_all(&script_dir);

    // Ensure toast PS1 exists (also used by test_toast)
    if toast {
        toast_command("Claude Code", "");
    }

    #[cfg(feature = "future_happy")]
    let happy_path = get_happy_path().to_string_lossy().replace('\\', "/");
    let toast_ps1 = script_dir.join("claude-notify-toast.ps1")
        .to_string_lossy().replace('\\', "/");
    let stop_s = stop_sound.replace('\\', "/");
    let ask_s = ask_sound.replace('\\', "/");
    let wh = webhook.replace('\'', "");

    let content = r#"'use strict';
// Generated by Claude Notify — do not edit manually
const cp=require('child_process');
const path=require('path');
const STOP_SOUND='__STOP_SOUND__';
const ASK_SOUND='__ASK_SOUND__';
const TOAST=__TOAST__;
const TOAST_PS1='__TOAST_PS1__';
// FUTURE_HAPPY: const HAPPY=__HAPPY__; const HP='__HAPPY_PATH__';
const GCHAT='__GCHAT__';
const ev=process.argv[2]||'stop';
const tm=process.argv[3]||'Claude Code';
let d='';
process.stdin.setEncoding('utf8');
process.stdin.on('data',c=>d+=c);
process.stdin.on('end',()=>{
  let project='',body='';
  try{
    const j=JSON.parse(d);
    if(j.cwd) project=path.basename(j.cwd);
    if(ev==='stop'){
      body=j.last_assistant_message?j.last_assistant_message.substring(0,200).replace(/[\r\n]+/g,' '):'Task completed';
    }else if(ev==='pre_tool_use'){
      if(j.tool_input){
        const q=j.tool_input.questions;
        if(Array.isArray(q)&&q.length>0) body=q[0].question||'Needs your input';
        else body=j.tool_input.question||'Needs your input';
      }else body='Needs your input';
    }else if(ev==='notification'){
      body=j.message||'Needs attention';
    }else if(ev==='permission_request'){
      body=j.tool_name?'Permission for '+j.tool_name:'Permission needed';
    }
  }catch{}
  const L={stop:'Done',pre_tool_use:'Question',notification:'Alert',permission_request:'Permission'};
  const title=project?'['+project+'] '+(L[ev]||'Claude Code'):L[ev]||'Claude Code';
  const msg=(body||'Claude Code').replace(/"/g,'').replace(/[\r\n]+/g,' ').substring(0,200);
  // Sound
  const snd=ev==='stop'?STOP_SOUND:ASK_SOUND;
  if(snd)try{cp.execSync('powershell.exe -WindowStyle Hidden -c "try{(New-Object Media.SoundPlayer \''+snd+'\').PlaySync()}catch{}"',{timeout:5000});}catch{}
  // Toast — must use exec() (not spawn detached) to inherit desktop session for WinRT toast
  if(TOAST&&TOAST_PS1)cp.exec('powershell.exe -NoProfile -ExecutionPolicy Bypass -File "'+TOAST_PS1+'" -Title "'+title.replace(/"/g,'')+'" -Message "'+tm.replace(/"/g,'')+'"',{timeout:10000},()=>{});
  // FUTURE_HAPPY: if(HAPPY&&HP)try{cp.execSync('"'+HP+'" notify -t "'+title.replace(/"/g,'')+'" -p "'+msg+'"',{timeout:10000,stdio:'ignore'});}catch{}
  // GChat
  if(GCHAT){
    const iu={stop:'https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/2705.png',pre_tool_use:'https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/2753.png',notification:'https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/1f514.png',permission_request:'https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/1f512.png'};
    const ic={stop:'BOOKMARK',pre_tool_use:'PERSON',notification:'DESCRIPTION',permission_request:'PERSON'};
    const card={cardsV2:[{cardId:'claude-notify',card:{header:{title,subtitle:msg.substring(0,200),imageUrl:iu[ev]||'',imageType:'CIRCLE'},sections:[{widgets:[{decoratedText:{startIcon:{knownIcon:ic[ev]||'BOOKMARK'},text:msg.substring(0,300)}}]}]}}]};
    fetch(GCHAT,{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(card)}).catch(()=>{});
  }
});
setTimeout(()=>process.exit(0),15000);
"#;

    let content = content
        .replace("__STOP_SOUND__", &stop_s)
        .replace("__ASK_SOUND__", &ask_s)
        .replace("__TOAST__", if toast { "true" } else { "false" })
        .replace("__TOAST_PS1__", &toast_ps1)
        .replace("__GCHAT__", &wh);

    // FUTURE_HAPPY: also call:
    //   .replace("__HAPPY__", if happy { "true" } else { "false" })
    //   .replace("__HAPPY_PATH__", &happy_path)

    let wrapper_path = script_dir.join("claude-notify-hook.cjs");
    let _ = fs::write(&wrapper_path, content);
}

/// Build hook command: just invokes the combined Node.js script with event type
fn build_cn_hook_cmd(event_type: &str, toast_msg: &str) -> Vec<Value> {
    let script_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let wrapper = script_dir.join("claude-notify-hook.cjs")
        .to_string_lossy().replace('\\', "/");
    let cmd = format!(
        "node \"{}\" \"{}\" \"{}\"",
        wrapper, event_type, toast_msg
    );
    vec![serde_json::json!({ "type": "command", "command": cmd })]
}

fn build_stop_entry() -> Value {
    let hooks = build_cn_hook_cmd("stop", "Claude Code finished a task");
    serde_json::json!({ "matcher": "", "hooks": hooks })
}

fn build_pre_tool_use_entry() -> Value {
    let hooks = build_cn_hook_cmd("pre_tool_use", "Claude Code is asking a question");
    serde_json::json!({ "matcher": "AskUserQuestion", "hooks": hooks })
}

fn build_notification_entry() -> Value {
    let hooks = build_cn_hook_cmd("notification", "Claude Code needs attention");
    serde_json::json!({ "matcher": "", "hooks": hooks })
}

fn build_permission_request_entry() -> Value {
    let hooks = build_cn_hook_cmd("permission_request", "Claude Code needs permission");
    serde_json::json!({ "matcher": "", "hooks": hooks })
}

// ── Tauri command handlers ────────────────────────────────────

#[tauri::command]
fn get_config() -> Config {
    let s = read_settings();

    // Prefer saved config, fall back to legacy command parsing
    if let Some(saved) = load_saved_config(&s) {
        return Config {
            enabled: s.get("hooks").is_some(),
            sound_path: saved.sound_path,
            ask_sound_path: saved.ask_sound_path,
            gchat_webhook: saved.gchat_webhook,
            auto_start: get_auto_start_enabled(),
            toast_enabled: saved.toast_enabled,
            #[cfg(feature = "future_happy")]
            happy_enabled: saved.happy_enabled,
            #[cfg(feature = "future_happy")]
            happy_auto_daemon: saved.happy_auto_daemon,
            #[cfg(feature = "future_happy")]
            happy_projects: saved.happy_projects,
        };
    }

    // Legacy fallback
    Config {
        enabled: s.get("hooks").is_some(),
        sound_path: extract_sound_from_hooks(&s, "Stop")
            .unwrap_or_else(|| "C:\\Windows\\Media\\notify.wav".to_string()),
        ask_sound_path: extract_sound_from_hooks(&s, "PreToolUse")
            .unwrap_or_else(|| "C:\\Windows\\Media\\Ring01.wav".to_string()),
        gchat_webhook: extract_gchat_from_hooks(&s).unwrap_or_default(),
        auto_start: get_auto_start_enabled(),
        toast_enabled: detect_cmd_in_hooks(&s, |cmd| cmd.contains("ToastNotificationManager")),
        #[cfg(feature = "future_happy")]
        happy_enabled: detect_cmd_in_hooks(&s, |cmd| {
            (cmd.contains("happy") && cmd.contains("notify"))
            || cmd.contains("claude-notify-happy.cjs")
        }),
        #[cfg(feature = "future_happy")]
        happy_auto_daemon: false,
        #[cfg(feature = "future_happy")]
        happy_projects: vec![],
    }
}

#[tauri::command]
fn save_config(args: SaveConfigArgs) -> Value {
    let mut s = read_settings();

    set_auto_start(args.auto_start);

    // Save our own config for reliable round-trip
    write_saved_config(&mut s, &SavedNotifyConfig::from(&args));

    if args.enabled {
        // Ensure hooks object exists
        if s.get("hooks").is_none() {
            if let Some(obj) = s.as_object_mut() {
                obj.insert("hooks".to_string(), Value::Object(Default::default()));
            }
        }

        // Generate the combined hook script once (embeds all config)
        generate_hook_script(
            &args.sound_path,
            &args.ask_sound_path,
            &args.gchat_webhook,
            args.toast_enabled,
            #[cfg(feature = "future_happy")]
            args.happy_enabled,
        );

        #[cfg(feature = "future_happy")]
        if args.happy_enabled {
            generate_happy_wrapper();
        }

        let stop_entry = build_stop_entry();
        let pre_entry = build_pre_tool_use_entry();
        let notif_entry = build_notification_entry();
        let permission_entry = build_permission_request_entry();

        if let Some(hooks) = s.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            let existing_stop = hooks.get("Stop").cloned().unwrap_or(Value::Array(vec![]));
            let existing_pre = hooks.get("PreToolUse").cloned().unwrap_or(Value::Array(vec![]));
            let existing_notif = hooks.get("Notification").cloned().unwrap_or(Value::Array(vec![]));
            let existing_pr = hooks.get("PermissionRequest").cloned().unwrap_or(Value::Array(vec![]));

            hooks.insert("Stop".to_string(), merge_cn_entry(&existing_stop, stop_entry));
            hooks.insert("PreToolUse".to_string(), merge_cn_entry(&existing_pre, pre_entry));
            hooks.insert("Notification".to_string(), merge_cn_entry(&existing_notif, notif_entry));
            hooks.insert("PermissionRequest".to_string(), merge_cn_entry(&existing_pr, permission_entry));

            // Clean up old SessionEnd hook if present
            if let Some(arr) = hooks.get("SessionEnd").and_then(|v| v.as_array()) {
                let remaining = filter_out_cn_entries(arr);
                if remaining.is_empty() {
                    hooks.remove("SessionEnd");
                } else {
                    hooks.insert("SessionEnd".to_string(), Value::Array(remaining));
                }
            }
        }
    } else {
        // Disable: remove only CN hooks, keep others
        if let Some(hooks) = s.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for key in &["Stop", "PreToolUse", "Notification", "SessionEnd", "PermissionRequest"] {
                if let Some(arr) = hooks.get(*key).and_then(|v| v.as_array()) {
                    let remaining = filter_out_cn_entries(arr);
                    if remaining.is_empty() {
                        hooks.remove(*key);
                    } else {
                        hooks.insert(key.to_string(), Value::Array(remaining));
                    }
                }
            }
            // Remove hooks object entirely if empty
            if hooks.is_empty() {
                if let Some(obj) = s.as_object_mut() {
                    obj.remove("hooks");
                }
            }
        }
    }

    write_settings(&s);
    serde_json::json!({ "ok": true })
}

#[tauri::command]
fn test_sound(path: String) -> Value {
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
fn test_gchat(webhook: String) -> Value {
    let json_body = gchat_card_json(
        "Test",
        "Test from Claude Notify app",
        "https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/72x72/1f9ea.png",
        "DESCRIPTION",
    );
    let ps_cmd = format!(
        "Invoke-RestMethod -Uri '{}' -Method POST -ContentType 'application/json' -Body '{}'",
        webhook, json_body
    );
    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-c", &ps_cmd])
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
fn test_toast() -> Value {
    let ps_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("claude-notify-toast.ps1");

    // Ensure the toast script exists (toast_command also writes it, but test may run standalone)
    if !ps_path.exists() {
        // Generate it by calling toast_command (which writes the file as side effect)
        toast_command("Claude Code", "Test notification");
    }

    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args([
            "-WindowStyle", "Hidden",
            "-ExecutionPolicy", "Bypass",
            "-File", &ps_path.to_string_lossy(),
            "-Title", "Claude Code",
            "-Message", "Test notification",
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

// ── Happy integration commands ────────────────────────────────

#[cfg(feature = "future_happy")]
fn find_npm() -> Option<PathBuf> {
    // 1. System Node.js install (most common)
    let system = PathBuf::from(r"C:\Program Files\nodejs\npm.cmd");
    if system.exists() {
        return Some(system);
    }
    // 2. User AppData npm
    let user = dirs::home_dir()
        .unwrap_or_default()
        .join("AppData")
        .join("Roaming")
        .join("npm")
        .join("npm.cmd");
    if user.exists() {
        return Some(user);
    }
    // 3. Try PATH via `where npm.cmd`
    if let Ok(output) = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/c", "where npm.cmd"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().next() {
            let p = PathBuf::from(line.trim());
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(feature = "future_happy")]
fn find_node() -> Option<PathBuf> {
    let system = PathBuf::from(r"C:\Program Files\nodejs\node.exe");
    if system.exists() {
        return Some(system);
    }
    // Try PATH via cmd
    if let Ok(output) = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/c", "where node.exe"])
        .output()
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = stdout.lines().next() {
            let p = PathBuf::from(line.trim());
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(feature = "future_happy")]
fn check_node_installed() -> bool {
    find_node().is_some()
}

#[cfg(feature = "future_happy")]
#[derive(Serialize)]
pub struct HappyStatus {
    node_installed: bool,
    installed: bool,
    authenticated: bool,
    status_text: String,
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn get_happy_status() -> HappyStatus {
    let node_ok = check_node_installed();
    if !node_ok {
        return HappyStatus {
            node_installed: false,
            installed: false,
            authenticated: false,
            status_text: "Node.js not found".to_string(),
        };
    }

    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return HappyStatus {
            node_installed: true,
            installed: false,
            authenticated: false,
            status_text: "Not installed".to_string(),
        };
    }

    let output = Command::new(&happy_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["auth", "status"])
        .output();

    match output {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            let combined = format!("{}{}", stdout, stderr).to_lowercase();

            let authenticated = combined.contains("authenticated")
                && !combined.contains("not authenticated")
                && !combined.contains("unauthenticated");

            let status_text = if authenticated {
                "Connected".to_string()
            } else {
                "Not paired".to_string()
            };

            HappyStatus {
                node_installed: true,
                installed: true,
                authenticated,
                status_text,
            }
        }
        Err(_) => HappyStatus {
            node_installed: true,
            installed: true,
            authenticated: false,
            status_text: "Status check failed".to_string(),
        },
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn install_happy() -> Value {
    if !check_node_installed() {
        return serde_json::json!({
            "ok": false,
            "error": "Node.js is not installed. Please install Node.js >= 18 first."
        });
    }

    let npm_path = match find_npm() {
        Some(p) => p,
        None => {
            return serde_json::json!({
                "ok": false,
                "error": "npm not found. Ensure Node.js is installed and restart the app."
            });
        }
    };

    let output = Command::new(&npm_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["install", "-g", "happy-coder"])
        .output();

    match output {
        Ok(o) if o.status.success() => {
            // Verify installation
            if get_happy_path().exists() {
                serde_json::json!({ "ok": true })
            } else {
                serde_json::json!({
                    "ok": false,
                    "error": "Install completed but happy.cmd not found. Try restarting the app."
                })
            }
        }
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr).to_string();
            serde_json::json!({
                "ok": false,
                "error": if err.is_empty() { "npm install failed".to_string() } else { err }
            })
        }
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn pair_happy() -> Value {
    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return serde_json::json!({
            "ok": false,
            "error": "happy-coder not installed yet"
        });
    }

    // Open a terminal with `happy auth login` so user can scan QR
    let hp = happy_path.to_string_lossy().to_string();
    let result = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/c", "start", "", "cmd", "/k", &hp, "auth", "login"])
        .spawn();

    match result {
        Ok(_) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn get_happy_project_dir() -> String {
    let s = read_settings();
    load_saved_config(&s)
        .map(|c| c.happy_project_dir)
        .unwrap_or_default()
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn set_happy_project_dir(dir: String) {
    let mut s = read_settings();
    if let Some(mut cfg) = load_saved_config(&s) {
        cfg.happy_project_dir = dir;
        write_saved_config(&mut s, &cfg);
        write_settings(&s);
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn launch_happy_session(cwd: String) -> Value {
    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return serde_json::json!({
            "ok": false,
            "error": "happy-coder not installed. Run: npm install -g happy-coder"
        });
    }

    let dir = if cwd.is_empty() {
        dirs::home_dir().unwrap_or_default().to_string_lossy().to_string()
    } else {
        cwd.clone()
    };

    // Save last-used directory
    let mut s = read_settings();
    if let Some(mut cfg) = load_saved_config(&s) {
        cfg.happy_project_dir = dir.clone();
        write_saved_config(&mut s, &cfg);
        write_settings(&s);
    }

    let hp = happy_path.to_string_lossy().to_string();
    // Write a temp batch file to avoid cmd escaping issues
    let batch_dir = dirs::home_dir().unwrap_or_default().join(".claude");
    let _ = fs::create_dir_all(&batch_dir);
    let batch_path = batch_dir.join("claude-notify-launch-happy.cmd");
    let batch_content = format!("@echo off\ncd /d \"{}\"\n\"{}\"", dir, hp);
    let _ = fs::write(&batch_path, batch_content);

    let result = Command::new("cmd")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/c", "start", "", "cmd", "/k", &batch_path.to_string_lossy().to_string()])
        .spawn();

    match result {
        Ok(_) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn check_happy_running() -> Value {
    let ps_cmd = r#"(Get-CimInstance Win32_Process -Filter "Name='node.exe'" | Where-Object { $_.CommandLine -match 'happy' } | Measure-Object).Count"#;
    let output = Command::new("powershell.exe")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["-c", ps_cmd])
        .output();

    let running = match output {
        Ok(o) => {
            let count_str = String::from_utf8_lossy(&o.stdout).trim().to_string();
            count_str.parse::<i32>().unwrap_or(0) > 0
        }
        Err(_) => false,
    };

    serde_json::json!({ "running": running })
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn get_daemon_status() -> Value {
    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return serde_json::json!({ "running": false, "error": "happy-coder not installed" });
    }
    let output = Command::new(&happy_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["daemon", "status"])
        .output();
    match output {
        Ok(o) => {
            let combined = format!(
                "{}{}",
                String::from_utf8_lossy(&o.stdout),
                String::from_utf8_lossy(&o.stderr)
            ).to_lowercase();
            let running = combined.contains("running") && !combined.contains("not running");
            serde_json::json!({ "running": running })
        }
        Err(e) => serde_json::json!({ "running": false, "error": e.to_string() }),
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn start_daemon() -> Value {
    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return serde_json::json!({ "ok": false, "error": "happy-coder not installed" });
    }
    let result = Command::new(&happy_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["daemon", "start"])
        .spawn();
    match result {
        Ok(_) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "ok": false, "error": e.to_string() }),
    }
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn stop_daemon() -> Value {
    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return serde_json::json!({ "ok": false, "error": "happy-coder not installed" });
    }
    let output = Command::new(&happy_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["daemon", "stop"])
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

#[cfg(feature = "future_happy")]
#[tauri::command]
fn test_happy() -> Value {
    let happy_path = get_happy_path();
    if !happy_path.exists() {
        return serde_json::json!({
            "ok": false,
            "error": "happy-coder not installed. Run: npm install -g happy-coder"
        });
    }
    let output = Command::new(&happy_path)
        .creation_flags(CREATE_NO_WINDOW)
        .args(["notify", "-t", "Test", "-p", "Test from Claude Notify"])
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

// ── VSCode workspace detection ────────────────────────────────

#[derive(Serialize)]
pub struct VscodeWorkspace {
    path: String,
    name: String,
    last_used: u64, // seconds since epoch
}

#[tauri::command]
fn detect_vscode_workspaces() -> Vec<VscodeWorkspace> {
    let storage_dir = dirs::home_dir()
        .unwrap_or_default()
        .join("AppData")
        .join("Roaming")
        .join("Code")
        .join("User")
        .join("workspaceStorage");

    let mut workspaces: Vec<VscodeWorkspace> = Vec::new();

    if let Ok(entries) = fs::read_dir(&storage_dir) {
        for entry in entries.flatten() {
            let ws_json = entry.path().join("workspace.json");
            if !ws_json.exists() {
                continue;
            }
            if let Ok(content) = fs::read_to_string(&ws_json) {
                if let Ok(parsed) = serde_json::from_str::<Value>(&content) {
                    if let Some(folder_uri) = parsed.get("folder").and_then(|v| v.as_str()) {
                        // Decode file:/// URI to path
                        if let Some(path) = decode_file_uri(folder_uri) {
                            if std::path::Path::new(&path).exists() {
                                let name = std::path::Path::new(&path)
                                    .file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
                                    .to_string();

                                // Use parent dir modification time as proxy for "last used"
                                let last_used = entry
                                    .metadata()
                                    .ok()
                                    .and_then(|m| m.modified().ok())
                                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs())
                                    .unwrap_or(0);

                                workspaces.push(VscodeWorkspace { path, name, last_used });
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by last_used descending (most recent first)
    workspaces.sort_by(|a, b| b.last_used.cmp(&a.last_used));
    // Return top 10
    workspaces.truncate(10);
    workspaces
}

fn decode_file_uri(uri: &str) -> Option<String> {
    let path_str = uri.strip_prefix("file:///")?;
    // Percent-decode
    let decoded = percent_decode(path_str);
    // Convert forward slashes to backslashes for Windows, then normalize
    let normalized = decoded.replace('/', "\\");
    // Add drive letter colon back: e%3A -> E:
    Some(normalized)
}

fn percent_decode(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '%' {
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            } else {
                result.push('%');
                result.push_str(&hex);
            }
        } else {
            result.push(ch);
        }
    }
    result
}

// ── App Setup ─────────────────────────────────────────────────

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            #[cfg(feature = "future_happy")]
            let launch_happy = MenuItemBuilder::with_id("launch_happy", "Launch Happy Session").build(app)?;
            #[cfg(feature = "future_happy")]
            let daemon_item = MenuItemBuilder::with_id("toggle_daemon", "Happy Daemon: Start").build(app)?;
            let open_item = MenuItemBuilder::with_id("open", "Open Settings").build(app)?;
            let quit_item = MenuItemBuilder::with_id("quit", "Quit").build(app)?;
            #[cfg(feature = "future_happy")]
            let menu = MenuBuilder::new(app)
                .item(&launch_happy)
                .item(&daemon_item)
                .item(&open_item)
                .separator()
                .item(&quit_item)
                .build()?;
            #[cfg(not(feature = "future_happy"))]
            let menu = MenuBuilder::new(app)
                .item(&open_item)
                .separator()
                .item(&quit_item)
                .build()?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Claude Code Notifications")
                .menu(&menu)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    #[cfg(feature = "future_happy")]
                    "launch_happy" => {
                        let happy_path = get_happy_path();
                        if happy_path.exists() {
                            let s = read_settings();
                            let dir = load_saved_config(&s)
                                .map(|c| c.happy_project_dir)
                                .unwrap_or_default();
                            let cwd = if dir.is_empty() {
                                dirs::home_dir().unwrap_or_default().to_string_lossy().to_string()
                            } else {
                                dir
                            };
                            let hp = happy_path.to_string_lossy().to_string();
                            let batch_dir = dirs::home_dir().unwrap_or_default().join(".claude");
                            let _ = std::fs::create_dir_all(&batch_dir);
                            let batch_path = batch_dir.join("claude-notify-launch-happy.cmd");
                            let batch_content = format!("@echo off\ncd /d \"{}\"\n\"{}\"", cwd, hp);
                            let _ = std::fs::write(&batch_path, batch_content);
                            let _ = Command::new("cmd")
                                .creation_flags(CREATE_NO_WINDOW)
                                .args(["/c", "start", "", "cmd", "/k", &batch_path.to_string_lossy().to_string()])
                                .spawn();
                        }
                    }
                    #[cfg(feature = "future_happy")]
                    "toggle_daemon" => {
                        let happy_path = get_happy_path();
                        if happy_path.exists() {
                            let status = Command::new(&happy_path)
                                .creation_flags(CREATE_NO_WINDOW)
                                .args(["daemon", "status"])
                                .output()
                                .map(|o| {
                                    let s = format!(
                                        "{}{}",
                                        String::from_utf8_lossy(&o.stdout),
                                        String::from_utf8_lossy(&o.stderr)
                                    ).to_lowercase();
                                    s.contains("running") && !s.contains("not running")
                                })
                                .unwrap_or(false);
                            let args = if status { vec!["daemon", "stop"] } else { vec!["daemon", "start"] };
                            let _ = Command::new(&happy_path)
                                .creation_flags(CREATE_NO_WINDOW)
                                .args(&args)
                                .spawn();
                        }
                    }
                    "open" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.show();
                            let _ = win.unminimize();
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
                                let _ = win.unminimize();
                                let _ = win.set_focus();
                            }
                        }
                    }
                })
                .build(app)?;

            if let Some(win) = app.get_webview_window("main") {
                let win_clone = win.clone();
                win.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = win_clone.hide();
                    }
                });
            }

            // Background thread: monitor Happy daemon and update tray tooltip
            #[cfg(feature = "future_happy")]
            {
                let app_handle = app.handle().clone();
                std::thread::spawn(move || {
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(10));
                        let happy_path = get_happy_path();
                        if !happy_path.exists() { continue; }
                        let running = Command::new(&happy_path)
                            .creation_flags(CREATE_NO_WINDOW)
                            .args(["daemon", "status"])
                            .output()
                            .map(|o| {
                                let s = format!(
                                    "{}{}",
                                    String::from_utf8_lossy(&o.stdout),
                                    String::from_utf8_lossy(&o.stderr)
                                ).to_lowercase();
                                s.contains("running") && !s.contains("not running")
                            })
                            .unwrap_or(false);

                        let tooltip = if running {
                            "Claude Notify — Happy daemon running"
                        } else {
                            "Claude Code Notifications"
                        };
                        if let Some(tray) = app_handle.tray_by_id("main-tray") {
                            let _ = tray.set_tooltip(Some(tooltip));
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_config,
            save_config,
            test_sound,
            test_gchat,
            test_toast,
            detect_vscode_workspaces,
            // FUTURE_HAPPY commands registered below when feature is enabled:
            #[cfg(feature = "future_happy")]
            test_happy,
            #[cfg(feature = "future_happy")]
            get_happy_status,
            #[cfg(feature = "future_happy")]
            install_happy,
            #[cfg(feature = "future_happy")]
            pair_happy,
            #[cfg(feature = "future_happy")]
            get_happy_project_dir,
            #[cfg(feature = "future_happy")]
            set_happy_project_dir,
            #[cfg(feature = "future_happy")]
            launch_happy_session,
            #[cfg(feature = "future_happy")]
            check_happy_running,
            #[cfg(feature = "future_happy")]
            get_daemon_status,
            #[cfg(feature = "future_happy")]
            start_daemon,
            #[cfg(feature = "future_happy")]
            stop_daemon,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
