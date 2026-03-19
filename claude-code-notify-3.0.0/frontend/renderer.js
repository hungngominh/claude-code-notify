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
const happyProjectDir   = document.getElementById('happyProjectDir');
const browseHappyDirBtn = document.getElementById('browseHappyDirBtn');
const launchHappyBtn    = document.getElementById('launchHappyBtn');
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

  // Load Happy project directory
  const dir = await invoke('get_happy_project_dir');
  if (dir) happyProjectDir.value = dir;

  // Check Happy setup status
  checkHappyStatus();
  checkHappyRunning();
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
          setTimeout(checkHappyRunning, 5000);
        } else {
          showStatus(res.error || 'Failed to launch', 'err', xIcon());
        }
      });
    });
  } catch {
    workspaceList.innerHTML = '<div class="ws-empty">Could not detect workspaces</div>';
  }
}

// ── Happy session monitor ─────────────────────────────────────

async function checkHappyRunning() {
  try {
    const res = await invoke('check_happy_running');
    happyRunningDot.className = 'happy-status-dot ' + (res.running ? 'connected' : '');
    happyRunningText.textContent = res.running ? 'Happy session active' : 'No active session';
  } catch {
    happyRunningDot.className = 'happy-status-dot';
    happyRunningText.textContent = 'No active session';
  }
}

setInterval(checkHappyRunning, 30000);

// ── Happy session launcher ────────────────────────────────────

browseHappyDirBtn.addEventListener('click', async () => {
  const picked = await open({
    title: 'Select project directory for Happy session',
    directory: true,
    multiple: false,
  });
  if (picked) {
    happyProjectDir.value = picked;
    await invoke('set_happy_project_dir', { dir: picked });
  }
});

launchHappyBtn.addEventListener('click', async () => {
  const dir = happyProjectDir.value.trim();
  if (!dir) {
    showStatus('Select a project directory first', 'err', xIcon());
    return;
  }
  launchHappyBtn.disabled = true;
  showStatus('Launching Happy session...', 'info', spinnerIcon());
  const res = await invoke('launch_happy_session', { cwd: dir });
  launchHappyBtn.disabled = false;
  if (res.ok) {
    showStatus('Happy session launched in new terminal', 'ok', checkIcon(), 6000);
    setTimeout(checkHappyRunning, 5000);
  } else {
    showStatus(res.error || 'Failed to launch', 'err', xIcon());
  }
});

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
