const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

const toggle          = document.getElementById('toggleInput');
const autoStart       = document.getElementById('autoStartInput');
const toastInput      = document.getElementById('toastInput');
const happyInput      = document.getElementById('happyInput');
const soundPath       = document.getElementById('soundPath');
const askSoundPath    = document.getElementById('askSoundPath');
const soundEnabled    = document.getElementById('soundEnabled');
const askSoundEnabled = document.getElementById('askSoundEnabled');
const soundRow        = document.getElementById('soundRow');
const askSoundRow     = document.getElementById('askSoundRow');
const gchatWebhook    = document.getElementById('gchatWebhook');
const gchatEnabled    = document.getElementById('gchatEnabled');
const gchatRow        = document.getElementById('gchatRow');
const browseBtn       = document.getElementById('browseBtn');
const browseAskBtn    = document.getElementById('browseAskBtn');
const testSoundBtn    = document.getElementById('testSoundBtn');
const testAskSoundBtn = document.getElementById('testAskSoundBtn');
const testGchatBtn    = document.getElementById('testGchatBtn');
const testHappyBtn    = document.getElementById('testHappyBtn');
const testToastBtn    = document.getElementById('testToastBtn');
const saveBtn         = document.getElementById('saveBtn');
const statusBar       = document.getElementById('statusBar');
const statusTxt       = document.getElementById('statusText');
const statusIco       = document.getElementById('statusIcon');
const statusDot       = document.getElementById('statusDot');

// Happy setup elements
const stepNodeDot     = document.getElementById('stepNodeDot');
const stepNodeText    = document.getElementById('stepNodeText');
const stepInstallDot  = document.getElementById('stepInstallDot');
const stepInstallText = document.getElementById('stepInstallText');
const installHappyBtn = document.getElementById('installHappyBtn');
const stepPairDot     = document.getElementById('stepPairDot');
const stepPairText    = document.getElementById('stepPairText');
const pairHappyBtn    = document.getElementById('pairHappyBtn');
const happyToggleRow  = document.getElementById('happyToggleRow');
const happySessionSection = document.getElementById('happySessionSection');

// Happy session elements
const happyProjectList   = document.getElementById('happyProjectList');
const addHappyProjectBtn = document.getElementById('addHappyProjectBtn');
const happyRunningDot   = document.getElementById('happyRunningDot');
const happyRunningText  = document.getElementById('happyRunningText');
const workspaceList     = document.getElementById('workspaceList');

let statusTimer = null;

function syncDot(enabled) {
  statusDot.classList.toggle('off', !enabled);
}

function syncSoundRow(enabled, row) {
  row.style.opacity = enabled ? '1' : '0.35';
  row.querySelectorAll('button').forEach(b => b.disabled = !enabled);
}

window.addEventListener('DOMContentLoaded', async () => {
  const cfg = await invoke('get_config');
  toggle.checked      = cfg.enabled;
  autoStart.checked   = cfg.auto_start;
  toastInput.checked  = cfg.toast_enabled;
  happyInput.checked  = cfg.happy_enabled;
  soundPath.value     = cfg.sound_path;
  askSoundPath.value  = cfg.ask_sound_path || '';
  gchatWebhook.value  = cfg.gchat_webhook || '';

  // Sound toggles — on = path is non-empty
  soundEnabled.checked    = !!cfg.sound_path;
  askSoundEnabled.checked = !!cfg.ask_sound_path;
  syncSoundRow(soundEnabled.checked, soundRow);
  syncSoundRow(askSoundEnabled.checked, askSoundRow);

  // GChat toggle — on = webhook is non-empty
  gchatEnabled.checked = !!cfg.gchat_webhook;
  syncSoundRow(gchatEnabled.checked, gchatRow);

  syncDot(cfg.enabled);

  await renderHappyProjects();

  // Check Happy setup status
  checkHappyStatus();
});

toggle.addEventListener('change', () => syncDot(toggle.checked));
soundEnabled.addEventListener('change',    () => syncSoundRow(soundEnabled.checked, soundRow));
askSoundEnabled.addEventListener('change', () => syncSoundRow(askSoundEnabled.checked, askSoundRow));
gchatEnabled.addEventListener('change',    () => syncSoundRow(gchatEnabled.checked, gchatRow));

// ── Happy setup check ─────────────────────────────────────────

function setStep(dot, text, state, label) {
  dot.className = 'happy-status-dot ' + state;
  text.textContent = label;
}

async function checkHappyStatus() {
  try {
    const s = await invoke('get_happy_status');

    // Step 1: Node.js
    if (s.node_installed) {
      setStep(stepNodeDot, stepNodeText, 'connected', 'Installed');
    } else {
      setStep(stepNodeDot, stepNodeText, 'error', 'Not found — install nodejs.org');
      setStep(stepInstallDot, stepInstallText, '', 'Waiting for Node.js');
      setStep(stepPairDot, stepPairText, '', 'Waiting');
      installHappyBtn.style.display = 'none';
      pairHappyBtn.style.display = 'none';
      happyToggleRow.style.display = 'none';
      happySessionSection.style.display = 'none';
      return;
    }

    // Step 2: happy-coder installed
    if (s.installed) {
      setStep(stepInstallDot, stepInstallText, 'connected', 'Installed');
      installHappyBtn.style.display = 'none';
    } else {
      setStep(stepInstallDot, stepInstallText, 'warning', 'Not installed');
      installHappyBtn.style.display = '';
      setStep(stepPairDot, stepPairText, '', 'Waiting for install');
      pairHappyBtn.style.display = 'none';
      happyToggleRow.style.display = 'none';
      happySessionSection.style.display = 'none';
      return;
    }

    // Step 3: Paired
    if (s.authenticated) {
      setStep(stepPairDot, stepPairText, 'connected', 'Connected');
      pairHappyBtn.style.display = 'none';
    } else {
      setStep(stepPairDot, stepPairText, 'warning', 'Not paired');
      pairHappyBtn.style.display = '';
    }

    // Show toggle + session launcher when installed
    happyToggleRow.style.display = 'flex';
    happySessionSection.style.display = '';

    // Load VSCode workspaces for quick-pick
    loadWorkspaces();

  } catch {
    setStep(stepNodeDot, stepNodeText, 'error', 'Check failed');
  }
}

async function renderHappyProjects() {
  if (!happyProjectList) return;
  let projects = [];
  try { projects = await invoke('get_happy_projects'); } catch {}
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
        <button class="btn-icon" title="Launch session" data-path="${p}">&#9654;</button>
        <button class="btn-icon" title="Remove" data-path="${p}" data-action="remove">&#x2715;</button>
      </div>
    `;
    el.querySelector('[title="Launch session"]').addEventListener('click', async (e) => {
      const path = e.currentTarget.dataset.path;
      try {
        const r = await invoke('launch_happy_session', { cwd: path });
        showStatus(r.ok ? 'Session launched!' : (r.error || 'Launch failed'), r.ok ? 'ok' : 'error');
      } catch (err) {
        showStatus('Launch failed: ' + err, 'error');
      }
    });
    el.querySelector('[data-action="remove"]').addEventListener('click', async (e) => {
      const path = e.currentTarget.dataset.path;
      try {
        await invoke('remove_happy_project', { path });
        await renderHappyProjects();
      } catch {}
    });
    happyProjectList.appendChild(el);
  });
}

async function checkDaemonStatus() {
  const dot = document.getElementById('happyRunningDot');
  const text = document.getElementById('happyRunningText');
  if (!dot || !text) return;
  try {
    const r = await invoke('get_daemon_status');
    dot.className = 'happy-status-dot ' + (r.running ? 'connected' : 'error');
    text.textContent = r.running ? 'Daemon running' : 'Daemon stopped';
  } catch {
    dot.className = 'happy-status-dot error';
    text.textContent = 'Status unknown';
  }
}

setInterval(checkDaemonStatus, 10000);
checkDaemonStatus();

// ── Install happy-coder ───────────────────────────────────────

installHappyBtn.addEventListener('click', async () => {
  installHappyBtn.disabled = true;
  installHappyBtn.textContent = 'Installing...';
  setStep(stepInstallDot, stepInstallText, 'warning', 'Installing...');
  showStatus('Running npm install -g happy-coder...', 'info', spinnerIcon(), 30000);

  const res = await invoke('install_happy');

  installHappyBtn.disabled = false;
  installHappyBtn.textContent = 'Install';

  if (res.ok) {
    showStatus('happy-coder installed!', 'ok', checkIcon());
    // Re-check everything
    await checkHappyStatus();
  } else {
    setStep(stepInstallDot, stepInstallText, 'error', 'Install failed');
    showStatus(res.error || 'Install failed', 'err', xIcon(), 8000);
  }
});

// ── Pair device ───────────────────────────────────────────────

pairHappyBtn.addEventListener('click', async () => {
  pairHappyBtn.disabled = true;
  showStatus('Opening terminal — scan the QR code with Happy app', 'info', spinnerIcon(), 10000);

  const res = await invoke('pair_happy');

  pairHappyBtn.disabled = false;

  if (res.ok) {
    showStatus('Scan the QR code in the terminal, then click Pair again to verify', 'ok', checkIcon(), 10000);
    // Wait a bit then re-check
    setTimeout(checkHappyStatus, 15000);
  } else {
    showStatus(res.error || 'Failed to open pairing', 'err', xIcon());
  }
});

// ── VSCode workspace detection ────────────────────────────────

async function loadWorkspaces() {
  try {
    const list = await invoke('detect_vscode_workspaces');
    if (!list || list.length === 0) {
      workspaceList.innerHTML = '<div class="ws-empty">No VSCode workspaces detected</div>';
      return;
    }
    workspaceList.innerHTML = list.map(ws => `
      <div class="ws-item" data-path="${ws.path.replace(/"/g, '&quot;')}" title="${ws.path}">
        <span class="ws-name">${ws.name}</span>
        <span class="ws-path">${ws.path}</span>
        <button class="ws-launch" title="Launch Happy here">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
            <polygon points="5 3 19 12 5 21 5 3"/>
          </svg>
        </button>
      </div>
    `).join('');

    // Click handler for each workspace item
    workspaceList.querySelectorAll('.ws-item').forEach(item => {
      item.addEventListener('click', async () => {
        const dir = item.dataset.path;
        showStatus('Launching Happy session...', 'info', spinnerIcon());
        const res = await invoke('launch_happy_session', { cwd: dir });
        if (res.ok) {
          showStatus(`Happy launched — ${item.querySelector('.ws-name').textContent}`, 'ok', checkIcon(), 6000);
          setTimeout(checkDaemonStatus, 5000);
        } else {
          showStatus(res.error || 'Failed to launch', 'err', xIcon());
        }
      });
    });
  } catch {
    workspaceList.innerHTML = '<div class="ws-empty">Could not detect workspaces</div>';
  }
}

// ── Sound browsing ────────────────────────────────────────────

browseBtn.addEventListener('click', async () => {
  const picked = await open({
    title: 'Select a .wav sound file',
    defaultPath: 'C:\\Windows\\Media',
    filters: [{ name: 'WAV Audio', extensions: ['wav'] }],
    multiple: false,
  });
  if (picked) soundPath.value = picked;
});

browseAskBtn.addEventListener('click', async () => {
  const picked = await open({
    title: 'Select a .wav sound for AskUserQuestion',
    defaultPath: 'C:\\Windows\\Media',
    filters: [{ name: 'WAV Audio', extensions: ['wav'] }],
    multiple: false,
  });
  if (picked) askSoundPath.value = picked;
});

// ── Test buttons ──────────────────────────────────────────────

testSoundBtn.addEventListener('click', async () => {
  testSoundBtn.disabled = true;
  showStatus('Playing stop sound...', 'info', spinnerIcon());
  const res = await invoke('test_sound', { path: soundPath.value });
  testSoundBtn.disabled = false;
  if (res.ok) {
    showStatus('Sound played', 'ok', checkIcon());
  } else {
    showStatus('Invalid WAV file', 'err', xIcon());
  }
});

testAskSoundBtn.addEventListener('click', async () => {
  testAskSoundBtn.disabled = true;
  showStatus('Playing ask sound...', 'info', spinnerIcon());
  const res = await invoke('test_sound', { path: askSoundPath.value });
  testAskSoundBtn.disabled = false;
  if (res.ok) {
    showStatus('Sound played', 'ok', checkIcon());
  } else {
    showStatus('Invalid WAV file', 'err', xIcon());
  }
});

testGchatBtn.addEventListener('click', async () => {
  const webhook = gchatWebhook.value.trim();
  if (!webhook) {
    showStatus('Enter a Google Chat webhook first', 'err', xIcon());
    return;
  }
  testGchatBtn.disabled = true;
  showStatus('Sending test message...', 'info', spinnerIcon());
  const res = await invoke('test_gchat', { webhook });
  testGchatBtn.disabled = false;
  if (res.ok) {
    showStatus('Message sent to Google Chat', 'ok', checkIcon());
  } else {
    showStatus('Failed — check your webhook URL', 'err', xIcon());
  }
});

testHappyBtn.addEventListener('click', async () => {
  testHappyBtn.disabled = true;
  showStatus('Sending Happy notification...', 'info', spinnerIcon());
  const res = await invoke('test_happy');
  testHappyBtn.disabled = false;
  if (res.ok) {
    showStatus('Push sent to phone', 'ok', checkIcon());
  } else {
    showStatus(res.error || 'Happy notify failed', 'err', xIcon());
  }
});

testToastBtn.addEventListener('click', async () => {
  testToastBtn.disabled = true;
  showStatus('Showing toast notification...', 'info', spinnerIcon());
  const res = await invoke('test_toast');
  testToastBtn.disabled = false;
  if (res.ok) {
    showStatus('Toast shown', 'ok', checkIcon());
  } else {
    showStatus(res.error || 'Toast failed', 'err', xIcon());
  }
});

if (addHappyProjectBtn) {
  addHappyProjectBtn.addEventListener('click', async () => {
    try {
      const selected = await open({ directory: true, multiple: false, title: 'Select project folder' });
      if (selected) {
        await invoke('add_happy_project', { path: selected });
        await renderHappyProjects();
      }
    } catch (err) {
      showStatus('Failed to add project: ' + err, 'error');
    }
  });
}

// ── Save ──────────────────────────────────────────────────────

saveBtn.addEventListener('click', async () => {
  saveBtn.disabled = true;
  saveBtn.textContent = 'Saving...';
  const res = await invoke('save_config', {
    args: {
      enabled:        toggle.checked,
      auto_start:     autoStart.checked,
      toast_enabled:  toastInput.checked,
      happy_enabled:  happyInput.checked,
      sound_path:     soundEnabled.checked    ? soundPath.value    : '',
      ask_sound_path: askSoundEnabled.checked ? askSoundPath.value : '',
      gchat_webhook:  gchatEnabled.checked    ? gchatWebhook.value.trim() : '',
      happy_auto_daemon: false,
      happy_projects: [],
    }
  });
  saveBtn.disabled = false;
  saveBtn.textContent = 'Save Settings';
  if (res.ok) {
    showStatus('Saved! Hooks apply to new sessions. Restart existing ones to update.', 'ok', checkIcon(), 6000);
  } else {
    showStatus('Failed to save', 'err', xIcon());
  }
});

// ── Helpers ───────────────────────────────────────────────────

function showStatus(msg, type, iconHtml, timeout = 4000) {
  statusTxt.textContent = msg;
  statusIco.innerHTML   = iconHtml;
  statusBar.className   = `status-bar visible ${type}`;
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => statusBar.classList.remove('visible'), timeout);
}

function checkIcon()   { return '<polyline points="20 6 9 17 4 12"/>'; }
function xIcon()       { return '<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>'; }
function spinnerIcon() { return '<path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>'; }
