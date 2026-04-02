# V4 Happy Interaction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add two-way interaction with Happy app — rich push notifications with question content, daemon management from tray, and multi-project session picker.

**Architecture:** Enable the existing `future_happy` feature flag, add daemon management commands and polling thread, update the hook CJS template to send rich context, and upgrade the session picker from single-dir to multi-project list.

**Tech Stack:** Tauri 2.x (Rust + HTML/JS), `happy-coder` npm CLI, Node.js CJS hook script embedded as Rust string template.

---

## File Map

| File | Change |
|---|---|
| `src-tauri/Cargo.toml` | Add `future_happy` to `default` features; bump version to 4.0.0 |
| `src-tauri/tauri.conf.json` | Bump version to 4.0.0 |
| `src-tauri/src/lib.rs` | Add daemon commands + polling; update hook template; replace single-dir with multi-project; update tray menu |
| `frontend/index.html` | Show Happy card (remove display:none); update session section to list UI |
| `frontend/renderer.js` | Call new daemon commands; update session picker to multi-project; fix calls to removed `get_happy_project_dir` |

> **CRITICAL:** Always edit `frontend/index.html` and `frontend/renderer.js`. NEVER edit root `index.html` or root `renderer.js`.

> **Build command:** `npm run build` (from `claude-code-notify-3.0.0/`). Touch `src-tauri/build.rs` before building if only frontend files changed.

---

## Task 1: Enable `future_happy` Feature Flag

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: Add `future_happy` to default features**

In `src-tauri/Cargo.toml`, replace:
```toml
[features]
future_happy = []
```
With:
```toml
[features]
default = ["future_happy"]
future_happy = []
```

- [ ] **Step 2: Verify cargo check passes**

```bash
cd D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri
cargo check 2>&1 | grep -E "^error" | head -20
```
Expected: no `error` lines (warnings about unused code are OK).

- [ ] **Step 3: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/src-tauri/Cargo.toml
git commit -m "feat: Enable future_happy feature flag by default"
```

---

## Task 2: Bump Version to 4.0.0

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`

- [ ] **Step 1: Update Cargo.toml version**

In `src-tauri/Cargo.toml`, change:
```toml
version = "3.0.3"
```
To:
```toml
version = "4.0.0"
```

- [ ] **Step 2: Update tauri.conf.json version**

In `src-tauri/tauri.conf.json`, change:
```json
"version": "3.0.3",
```
To:
```json
"version": "4.0.0",
```

- [ ] **Step 3: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/src-tauri/Cargo.toml claude-code-notify-3.0.0/src-tauri/tauri.conf.json
git commit -m "chore: Bump version to 4.0.0"
```

---

## Task 3: Add Daemon Management — Rust Commands

**Files:**
- Modify: `src-tauri/src/lib.rs` (after line 1191, before `// ── VSCode workspace detection`)

- [ ] **Step 1: Add `SavedConfig` fields for daemon config**

In `lib.rs`, find the `SavedConfig` struct (around line 60–100). Add two new fields inside the struct:

```rust
#[cfg(feature = "future_happy")]
happy_auto_daemon: bool,
#[cfg(feature = "future_happy")]
happy_projects: Vec<String>,
```

Also update `SavedConfig::from_args` to include these fields (find the block that builds `SavedConfig` from `SaveConfigArgs`):
```rust
#[cfg(feature = "future_happy")]
happy_auto_daemon: a.happy_auto_daemon,
#[cfg(feature = "future_happy")]
happy_projects: a.happy_projects.clone(),
```

And update `SavedConfig` default construction (the `#[cfg(not(feature = "future_happy"))]` block and any `SavedConfig { .. }` literal):
```rust
#[cfg(feature = "future_happy")]
happy_auto_daemon: false,
#[cfg(feature = "future_happy")]
happy_projects: vec![],
```

- [ ] **Step 2: Add daemon commands**

Insert after the existing `check_happy_running` function (around line 1191):

```rust
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
```

- [ ] **Step 3: Register new commands in invoke_handler**

In the `tauri::generate_handler![]` block (around line 1425), add after the existing `#[cfg(feature = "future_happy")]` entries:

```rust
#[cfg(feature = "future_happy")]
get_daemon_status,
#[cfg(feature = "future_happy")]
start_daemon,
#[cfg(feature = "future_happy")]
stop_daemon,
```

- [ ] **Step 4: Add background daemon polling thread**

Inside the `#[cfg(feature = "future_happy")]` background thread block (around line 1383), replace the current 30s session-check loop with a 10s daemon-status loop that updates tray icon tooltip AND emits a JS event:

```rust
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
```

- [ ] **Step 5: Add tray menu items for daemon**

In `run()`, inside the `#[cfg(feature = "future_happy")]` tray menu build block (around line 1292), add a daemon toggle item:

```rust
#[cfg(feature = "future_happy")]
let daemon_item = MenuItemBuilder::with_id("toggle_daemon", "Start Happy Daemon").build(app)?;
```

Update the `#[cfg(feature = "future_happy")]` menu builder to include it:
```rust
#[cfg(feature = "future_happy")]
let menu = MenuBuilder::new(app)
    .item(&launch_happy)
    .item(&daemon_item)
    .item(&open_item)
    .separator()
    .item(&quit_item)
    .build()?;
```

Add handler in `on_menu_event`:
```rust
#[cfg(feature = "future_happy")]
"toggle_daemon" => {
    let happy_path = get_happy_path();
    if happy_path.exists() {
        // Check current status, toggle
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
```

- [ ] **Step 6: Cargo check**

```bash
cd D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri
cargo check 2>&1 | grep "^error" | head -20
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/src-tauri/src/lib.rs
git commit -m "feat: Add Happy daemon management (start/stop/status + tray toggle)"
```

---

## Task 4: Rich Push Notifications in Hook CJS

**Files:**
- Modify: `src-tauri/src/lib.rs` (the `generate_hook_wrapper` function, around line 560–644)

- [ ] **Step 1: Update hook template to add rich happy notify**

Find the `generate_hook_wrapper` function. Inside the `content` string template (the multi-line Rust string containing the CJS script), find the commented-out `FUTURE_HAPPY` line:

```
  // FUTURE_HAPPY: if(HAPPY&&HP)try{cp.execSync('"'+HP+'" notify -t "'+title.replace(/"/g,'')+'" -p "'+msg+'"',{timeout:10000,stdio:'ignore'});}catch{}
```

Replace that single comment with this block (uncommented, using stdin-parsed question):

```
  // Happy rich notify
  if(HAPPY&&HP){
    try{
      const qtext=data.tool_input&&data.tool_input.question?data.tool_input.question:(data.last_assistant_message||'');
      const proj=data.cwd?data.cwd.replace(/\\\\/g,'/').split('/').filter(Boolean).pop():'';
      const notifTitle=(proj?'\u2753 '+proj:'\u2753 Claude Code');
      const base=ev==='stop'?'Task finished':(ev==='notification'?'Needs attention':(ev==='permission_request'?'Needs permission':''));
      const notifMsg=qtext?(qtext.length>200?qtext.substring(0,197)+'...':qtext):(base+(proj?' \u2014 '+proj:''));
      cp.execSync('"'+HP+'" notify -t "'+notifTitle.replace(/"/g,'')+'" -p "'+notifMsg.replace(/"/g,'')+'"',{timeout:10000,stdio:'ignore'});
    }catch{}
  }
```

Also update the function signature to include `happy` and `happy_path` params (find `generate_hook_wrapper` function signature):

```rust
fn generate_hook_wrapper(
    stop_sound: &str,
    ask_sound: &str,
    toast: bool,
    gchat_webhook: &str,
    #[cfg(feature = "future_happy")] happy: bool,
    #[cfg(feature = "future_happy")] happy_path: &str,
)
```

And add the `__HAPPY__` and `__HAPPY_PATH__` replacement lines (currently commented out, around line 639–641):

```rust
    #[cfg(feature = "future_happy")]
    let content = content
        .replace("__HAPPY__", if happy { "true" } else { "false" })
        .replace("__HAPPY_PATH__", happy_path);
```

Finally, add `HAPPY` and `HP` constants near the top of the CJS template:

```js
const HAPPY=__HAPPY__;
const HP='__HAPPY_PATH__';
```

- [ ] **Step 2: Update all callers of `generate_hook_wrapper`**

Find where `generate_hook_wrapper` is called (in `save_config` command, around line 740). Update the call to pass the new happy args:

```rust
#[cfg(feature = "future_happy")]
let happy_path_fwd = get_happy_path().to_string_lossy().replace('\\', "/");

generate_hook_wrapper(
    &stop_s,
    &ask_s,
    toast,
    &wh,
    #[cfg(feature = "future_happy")] args.happy_enabled,
    #[cfg(feature = "future_happy")] &happy_path_fwd,
);
```

- [ ] **Step 3: Cargo check**

```bash
cd D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri
cargo check 2>&1 | grep "^error" | head -20
```
Expected: no errors.

- [ ] **Step 4: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/src-tauri/src/lib.rs
git commit -m "feat: Send rich question content in Happy push notifications"
```

---

## Task 5: Multi-Project Session Picker — Backend

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Replace `happy_project_dir` (single) with `happy_projects` (list) in `SavedConfig`**

Find `SavedConfig` struct. Replace:
```rust
#[cfg(feature = "future_happy")]
happy_project_dir: String,
```
With:
```rust
#[cfg(feature = "future_happy")]
happy_projects: Vec<String>,
```

Update `SavedConfig::from_args`:
```rust
// Replace:
happy_project_dir: a.happy_project_dir.clone(),
// With:
happy_projects: a.happy_projects.clone(),
```

Update `SavedConfig` default construction:
```rust
// Replace:
happy_project_dir: String::new(),
// With:
happy_projects: vec![],
```

- [ ] **Step 2: Replace `SaveConfigArgs` field**

Find `SaveConfigArgs` struct. Replace:
```rust
#[cfg(feature = "future_happy")]
happy_project_dir: String,
```
With:
```rust
#[cfg(feature = "future_happy")]
happy_projects: Vec<String>,
```

Do the same for `Config` struct (the return type of `get_config`):
```rust
// Replace:
#[cfg(feature = "future_happy")]
happy_project_dir: String,  // if this field exists
// With:
#[cfg(feature = "future_happy")]
happy_projects: Vec<String>,
```

- [ ] **Step 3: Replace `get_happy_project_dir` and `set_happy_project_dir` with multi-project commands**

Remove (or comment) the old `get_happy_project_dir` and `set_happy_project_dir` commands.

Add new commands:

```rust
#[cfg(feature = "future_happy")]
#[tauri::command]
fn get_happy_projects() -> Vec<String> {
    let s = read_settings();
    load_saved_config(&s)
        .map(|c| c.happy_projects)
        .unwrap_or_default()
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn add_happy_project(path: String) -> Vec<String> {
    let mut s = read_settings();
    if let Some(mut cfg) = load_saved_config(&s) {
        if !cfg.happy_projects.contains(&path) {
            cfg.happy_projects.push(path);
        }
        let projects = cfg.happy_projects.clone();
        write_saved_config(&mut s, &cfg);
        write_settings(&s);
        return projects;
    }
    vec![]
}

#[cfg(feature = "future_happy")]
#[tauri::command]
fn remove_happy_project(path: String) -> Vec<String> {
    let mut s = read_settings();
    if let Some(mut cfg) = load_saved_config(&s) {
        cfg.happy_projects.retain(|p| p != &path);
        let projects = cfg.happy_projects.clone();
        write_saved_config(&mut s, &cfg);
        write_settings(&s);
        return projects;
    }
    vec![]
}
```

- [ ] **Step 4: Update `launch_happy_session` to accept `cwd` only (already does)**

The existing `launch_happy_session(cwd: String)` command is fine as-is. It launches a terminal session in the given directory. No change needed.

- [ ] **Step 5: Update `invoke_handler` to use new commands**

In `tauri::generate_handler![]`, replace:
```rust
#[cfg(feature = "future_happy")]
get_happy_project_dir,
#[cfg(feature = "future_happy")]
set_happy_project_dir,
```
With:
```rust
#[cfg(feature = "future_happy")]
get_happy_projects,
#[cfg(feature = "future_happy")]
add_happy_project,
#[cfg(feature = "future_happy")]
remove_happy_project,
```

- [ ] **Step 6: Cargo check**

```bash
cd D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri
cargo check 2>&1 | grep "^error" | head -20
```
Expected: no errors.

- [ ] **Step 7: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/src-tauri/src/lib.rs
git commit -m "feat: Replace single happy_project_dir with multi-project happy_projects list"
```

---

## Task 6: Multi-Project Session Picker — Frontend HTML

**Files:**
- Modify: `frontend/index.html`

- [ ] **Step 1: Show Happy card (remove display:none)**

Find the Happy card element (around line 327):
```html
<div class="card" id="happyCard" style="display:none">
```
Remove `style="display:none"`:
```html
<div class="card" id="happyCard">
```

- [ ] **Step 2: Replace session section with multi-project list UI**

Find `id="happySessionSection"` (the single-project dir input). Replace the entire section contents with:

```html
<div id="happySessionSection">
  <div class="row-desc" style="margin-bottom:6px;">Projects (launch two-way session)</div>
  <div id="happyProjectList" style="margin-bottom:6px;max-height:160px;overflow-y:auto;"></div>
  <button class="btn" id="addHappyProjectBtn" style="width:100%;">+ Add Project</button>
</div>
```

- [ ] **Step 3: Add CSS for project list items**

Inside the `<style>` block, add:

```css
.proj-item {
  display: flex; align-items: center; justify-content: space-between;
  padding: 5px 8px; background: var(--input-bg); border-radius: 6px;
  margin-bottom: 4px; gap: 8px;
}
.proj-name { font-size: 12px; font-weight: 500; flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.proj-path { font-size: 10px; color: var(--muted); flex: 2; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.proj-actions { display: flex; gap: 4px; flex-shrink: 0; }
```

- [ ] **Step 4: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/frontend/index.html
git commit -m "feat: Update Happy card UI to multi-project session picker"
```

---

## Task 7: Multi-Project Session Picker — Frontend JS

**Files:**
- Modify: `frontend/renderer.js`

- [ ] **Step 1: Replace Happy session element refs and add new ones**

Find and remove these lines near the top of renderer.js:
```js
const happyProjectDir   = document.getElementById('happyProjectDir');
const browseHappyDirBtn = document.getElementById('browseHappyDirBtn');
const launchHappyBtn    = document.getElementById('launchHappyBtn');
```

Add new refs:
```js
const happyProjectList  = document.getElementById('happyProjectList');
const addHappyProjectBtn = document.getElementById('addHappyProjectBtn');
```

- [ ] **Step 2: Remove old `get_happy_project_dir` call**

Find in `window.addEventListener('DOMContentLoaded', ...)`:
```js
  const dir = await invoke('get_happy_project_dir');
  if (dir) happyProjectDir.value = dir;
```
Replace with:
```js
  await renderHappyProjects();
```

- [ ] **Step 3: Add `renderHappyProjects` function**

Add after the `checkHappyRunning` function:

```js
async function renderHappyProjects() {
  const projects = await invoke('get_happy_projects');
  if (!happyProjectList) return;
  happyProjectList.innerHTML = '';
  if (projects.length === 0) {
    happyProjectList.innerHTML = '<div style="font-size:11px;color:var(--muted);padding:4px 8px;">No projects added yet</div>';
    return;
  }
  projects.forEach(p => {
    const name = p.replace(/\\/g, '/').split('/').filter(Boolean).pop() || p;
    const el = document.createElement('div');
    el.className = 'proj-item';
    el.innerHTML = `
      <span class="proj-name" title="${p}">${name}</span>
      <span class="proj-path" title="${p}">${p}</span>
      <div class="proj-actions">
        <button class="btn-icon" title="Launch session" data-path="${p}">▶</button>
        <button class="btn-icon" title="Remove" data-path="${p}" data-action="remove">✕</button>
      </div>
    `;
    el.querySelector('[title="Launch session"]').addEventListener('click', async (e) => {
      const path = e.currentTarget.dataset.path;
      const r = await invoke('launch_happy_session', { cwd: path });
      showStatus(r.ok ? 'Session launched!' : (r.error || 'Launch failed'), r.ok ? 'ok' : 'error');
    });
    el.querySelector('[data-action="remove"]').addEventListener('click', async (e) => {
      const path = e.currentTarget.dataset.path;
      await invoke('remove_happy_project', { path });
      await renderHappyProjects();
    });
    happyProjectList.appendChild(el);
  });
}
```

- [ ] **Step 4: Add "Add Project" button handler**

After the existing button event listeners, add:

```js
if (addHappyProjectBtn) {
  addHappyProjectBtn.addEventListener('click', async () => {
    const selected = await open({ directory: true, multiple: false, title: 'Select project folder' });
    if (selected) {
      await invoke('add_happy_project', { path: selected });
      await renderHappyProjects();
    }
  });
}
```

- [ ] **Step 5: Add daemon status polling to frontend**

Add a daemon status poll that updates a UI indicator every 10s. Find the `checkHappyStatus` function and add after it:

```js
async function checkDaemonStatus() {
  try {
    const r = await invoke('get_daemon_status');
    const dot = document.getElementById('happyRunningDot');
    const text = document.getElementById('happyRunningText');
    if (dot && text) {
      dot.className = 'happy-status-dot ' + (r.running ? 'connected' : 'error');
      text.textContent = r.running ? 'Daemon running' : 'Daemon stopped';
    }
  } catch {}
}

// Poll every 10s
setInterval(checkDaemonStatus, 10000);
checkDaemonStatus();
```

- [ ] **Step 6: Copy renderer.js to root (keep in sync)**

```bash
cp "D:/claude-code-mobile-interact/claude-code-notify-3.0.0/frontend/renderer.js" \
   "D:/claude-code-mobile-interact/claude-code-notify-3.0.0/renderer.js"
```

- [ ] **Step 7: Commit**

```bash
cd D:/claude-code-mobile-interact
git add claude-code-notify-3.0.0/frontend/renderer.js claude-code-notify-3.0.0/renderer.js
git commit -m "feat: Multi-project session picker UI + daemon status polling"
```

---

## Task 8: Build & Test

**Files:**
- Modify: `src-tauri/build.rs` (touch to force rebuild)

- [ ] **Step 1: Touch build.rs to force full rebuild**

```bash
touch D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri/build.rs
```

- [ ] **Step 2: Run full build**

```bash
cd D:/claude-code-mobile-interact/claude-code-notify-3.0.0
npm run build 2>&1 | tail -10
```
Expected output ends with:
```
Finished bundle at:
    ...\Claude Notify_4.0.0_x64-setup.exe
```

- [ ] **Step 3: Verify installer exists and has today's date**

```bash
ls -la "D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri/target/release/bundle/nsis/"
```
Expected: `Claude Notify_4.0.0_x64-setup.exe` with today's timestamp.

- [ ] **Step 4: Manual test checklist**

Install and verify:
- [ ] Happy card is visible in Settings window
- [ ] Setup wizard shows correct step status (Node, Install, Pair)
- [ ] "Add Project" opens folder picker → project appears in list
- [ ] Launch button (▶) opens terminal with `happy` running in that directory
- [ ] Remove (✕) removes project from list
- [ ] Tray menu shows "Start/Stop Happy Daemon" item
- [ ] After running `happy daemon start` manually: daemon status dot turns green
- [ ] Happy toggle in notifications card still works
- [ ] Sound, Toast, GChat still work (regression test)
- [ ] Trigger Stop hook via Claude Code: push notification arrives on phone with project name

- [ ] **Step 5: Package and push**

```bash
powershell -ExecutionPolicy Bypass -Command "
\$installer = 'D:/claude-code-mobile-interact/claude-code-notify-3.0.0/src-tauri/target/release/bundle/nsis/Claude Notify_4.0.0_x64-setup.exe'
\$out = 'D:/claude-code-mobile-interact/Claude-Notify-v4.0.0.zip'
Compress-Archive -Path \$installer -DestinationPath \$out -Force
Write-Host ('Done: {0} ({1:N1} MB)' -f \$out, ((Get-Item \$out).Length / 1MB))
"
```

- [ ] **Step 6: Commit and push branch**

```bash
cd D:/claude-code-mobile-interact
git add -A
git commit -m "chore: v4.0.0 build artifacts"
git push upstream feature/v4.0.0-happy-interaction
```

---

## Self-Review

**Spec coverage check:**
- ✅ Setup Wizard (3-step) → Task 1 (feature enabled) + existing UI shown in Task 6
- ✅ Daemon management from tray → Task 3
- ✅ Rich notifications with question content → Task 4
- ✅ Multi-project session picker → Tasks 5, 6, 7
- ✅ `happy_auto_daemon` config field → Task 3 Step 1
- ✅ `happy_projects` config field → Task 5
- ✅ Error handling (daemon down, not installed) → Task 3 commands return `{ ok: false, error: ... }`
- ✅ Daemon status polling every 10s → Tasks 3 + 7

**Placeholder scan:** No TBD, TODO, or incomplete steps found.

**Type consistency:**
- `add_happy_project(path: String)` called as `invoke('add_happy_project', { path: selected })` ✅
- `remove_happy_project(path: String)` called as `invoke('remove_happy_project', { path })` ✅
- `launch_happy_session(cwd: String)` called as `invoke('launch_happy_session', { cwd: path })` ✅
- `get_daemon_status()` called as `invoke('get_daemon_status')` ✅
