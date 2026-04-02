# V4 Design: Two-Way Interaction via Happy App

**Date:** 2026-04-02  
**Version:** 4.0.0  
**Status:** Approved  

---

## Overview

V4 adds two-way interaction between Claude Code (desktop) and teammates via the Happy app on mobile. Teammates receive rich push notifications on their phone, can view the full conversation, and reply in Vietnamese (or any language) using their phone keyboard — bypassing the terminal Vietnamese input issues entirely.

---

## Section 1: Architecture

### Current (v3)
```
Claude Code (hooks) → claude-notify-hook.cjs → Sound + Toast + Google Chat
```

### V4
```
Claude Code (hooks) → claude-notify-hook.cjs → Sound + Toast + Google Chat
                                              → happy notify (rich push)
                                                    ↓
                                              Phone (Happy app)
                                                    ↓ reply
                                              Happy daemon (desktop)
                                                    ↓
                                              Claude Code (answer received)
```

### Components Added
| Component | Type | Responsibility |
|---|---|---|
| Happy Setup Wizard | Tauri UI card | Guide user: install → auth → API key |
| Daemon Manager | Rust + Tray | Start/stop/monitor `happy daemon` |
| Rich Hook | Node.js CJS | Parse stdin JSON → rich `happy notify` |
| Session Picker | Tauri UI card | List/start/stop Happy sessions per project |

### Tech Stack (unchanged)
- Tauri 2.x (Rust backend + HTML/JS frontend)
- `claude-notify-hook.cjs` — Node.js hook called by Claude Code
- `happy-coder` npm CLI — Happy app integration layer

---

## Section 2: Feature Details

### Feature 1: Happy Setup Wizard

Settings window card, guides user through 3 steps with auto-detection:

```
┌─────────────────────────────────────┐
│ 📱 Happy Setup                      │
│                                     │
│ Step 1: Install Happy    ● Done     │
│ Step 2: Login & Pair     ● Done     │
│ Step 3: Connect API Key  ○ Pending  │
│                                     │
│ [Connect API Key]                   │
│                                     │
│ Status: Paired ✅ | Daemon: Off ⚫  │
└─────────────────────────────────────┘
```

- **Step 1**: Check if `happy` command exists → if not, button runs `npm install -g happy-coder`
- **Step 2**: Check `happy auth status` → if not authed → open terminal, run `happy auth login` (QR scan on phone)
- **Step 3**: Check API key connected → `happy connect claude` (stores key in Happy cloud so phone can use it)
- Each step auto-detects status on app open and on manual refresh

### Feature 2: Daemon Management from Tray

Tray menu additions:
- **🟢 Happy Daemon: Running** / **🔴 Happy Daemon: Stopped** — status indicator
- **Start Daemon** / **Stop Daemon** — toggle action
- App polls `happy daemon status` every 10s to update tray icon

Rust implementation:
```rust
// Start daemon
Command::new("happy").args(["daemon", "start"]).spawn()

// Check status  
Command::new("happy").args(["daemon", "status"]).output()

// Stop daemon
Command::new("happy").args(["daemon", "stop"]).output()
```

Optional: `happy_auto_daemon: true` config — auto-start daemon when app launches.

### Feature 3: Rich Push Notifications

Updated `claude-notify-hook.cjs` — send actual question content to phone:

```js
// v3 (generic):
happy notify -p "Claude Code is asking a question"

// v4 (rich):
happy notify -t "❓ my-project" -p "Claude asks: Which database should we use? (PostgreSQL / MongoDB / SQLite)"
```

Implementation:
- Hook reads stdin JSON (Claude Code sends this on every hook call)
- Extract `tool_input.question` (AskUserQuestion content)
- Extract `cwd` → `Split-Path $cwd -Leaf` → project name
- Truncate message at 200 chars (push notification limit)
- Fallback: if `happy notify` fails (network/daemon down) → sound + toast still work (v3 behavior unchanged)

### Feature 4: Session / Project Picker

Settings window card:

```
┌─────────────────────────────────────┐
│ 📂 Happy Sessions                   │
│                                     │
│ ● project-a    Running  [Stop]      │
│ ● project-b    Running  [Stop]      │
│ ○ project-c    Stopped  [Start]     │
│                                     │
│ [+ Add Project...] → folder picker  │
└─────────────────────────────────────┘
```

- List sessions via `happy daemon list`
- Start session: POST to Happy daemon Fastify endpoint `/spawn-session` `{ cwd: "/path/to/project" }`
- Stop session: POST `/stop-session` `{ sessionId }`
- Add project: Tauri file dialog → save path to config → auto-spawn session

---

## Section 3: Data Flow & Error Handling

### Flow 1: Teammate away from desk — Claude asks a question

```
1. Claude (running via Happy/VSCode) asks a question
   ↓ PreToolUse:AskUserQuestion hook fires
2. claude-notify-hook.cjs reads stdin JSON
   ↓ extracts: question text, project name, session_id
3. Sound + Toast fire (local, immediate)
   ↓
4. happy notify -t "❓ project-name" -p "Claude asks: ..."
   ↓ Happy relay → phone push notification
5. Teammate reads question on phone, opens Happy app
   ↓ sees full conversation context
6. Types reply using phone keyboard (Vietnamese works fine here!)
   ↓ Happy relay → desktop Happy session
7. Claude receives answer, continues task
```

### Flow 2: Teammate starts session from phone

```
1. Teammate opens Happy app on phone
   ↓ sees list of machines / projects
2. Selects project → "Start session"
   ↓ RPC: spawn-happy-session
3. Happy daemon on desktop spawns Claude CLI
   ↓ claude --resume (or fresh session)
4. Phone shows chat UI → teammate types prompt → Claude runs
```

### Flow 3: Seamless handoff phone → VSCode

```
1. Happy session finished task via phone
2. Teammate opens VSCode on desktop
3. Claude Code can --resume same session
   (conversation history in ~/.claude/projects/ is shared)
4. Continue normally in VSCode
```

### Error Handling

| Scenario | Handling |
|---|---|
| `happy` not installed | Setup Wizard shows Step 1, Install button |
| `happy auth status` = not authed | Setup Wizard shows Step 2, Pair button |
| Daemon crashed | Tray icon → 🔴, option to restart |
| Daemon status check timeout | Retry 3×, then show "Unknown" |
| `happy notify` fails (network) | Fallback: sound + toast only (v3 behavior) |
| Session spawn fails | Toast error + log to `~/.claude/claude-notify.log` |
| Happy cloud relay down | Local notifications still work |

---

## Section 4: Implementation Phases

### Phase 1 — Re-enable Happy + Fix Format (1 day)
- Remove `#[cfg(feature = "future_happy")]` gate from lib.rs
- Polish Happy Setup Wizard UI (code already exists)
- Verify: `happy auth login`, `happy auth status` invoked correctly from Tauri
- Update hook format to `matcher + hooks[]` (already done in v3.x)

### Phase 2 — Daemon Management (1 day)
- Tauri commands: `start_daemon`, `stop_daemon`, `get_daemon_status`
- Tray menu: dynamic Start/Stop item + 🟢/🔴 status
- Poll daemon status every 10s
- Config option: `happy_auto_daemon` — auto-start on app launch

### Phase 3 — Rich Notifications (1 day)
- Update `claude-notify-hook.cjs`:
  - Parse stdin JSON → extract `tool_input.question`
  - Extract `cwd` → derive project name
  - Build message: truncate at 200 chars
  - Call `happy notify -t "..." -p "..."`
- Fallback if `happy notify` exits non-zero: continue silently

### Phase 4 — Session / Project Picker (1–2 days)
- UI card in Settings: list projects (from config), Add/Remove buttons
- Tauri command: `list_happy_sessions` → calls `happy daemon list`
- Tauri command: `start_happy_session(cwd)` → POST to Happy daemon
- Tauri command: `stop_happy_session(sessionId)` → POST to Happy daemon
- Persist project list in `_claudeNotify.happy_projects` config key

### Phase 5 — Test & Polish (1 day)
- Test full flow: wizard → daemon → session → phone reply → Claude continues
- Test multi-project scenarios
- Test all error cases (daemon down, network off, not authed)
- Build installer `Claude Notify_4.0.0_x64-setup.exe`

**Total estimate: 5–6 days**

Phases 2 and 3 can be done in parallel.

---

## Config Schema (additions)

Added to `_claudeNotify` section in `~/.claude/settings.json`:

```json
{
  "_claudeNotify": {
    "sound_path": "C:/Windows/Media/notify.wav",
    "sound_enabled": true,
    "ask_sound_path": "C:/Windows/Media/notify.wav",
    "ask_sound_enabled": true,
    "toast_enabled": true,
    "gchat_webhook": "https://chat.googleapis.com/...",
    "gchat_enabled": true,
    "happy_enabled": true,
    "happy_projects": [
      "D:/projects/project-a",
      "D:/projects/project-b"
    ],
    "happy_auto_daemon": true,
    "happy_rich_notify": true
  }
}
```

---

## Out of Scope (Future)

- Custom notification templates per hook type
- Happy session sharing between teammates
- Web UI for session management
- Mobile-initiated file uploads
