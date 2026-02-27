const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

const toggle          = document.getElementById('toggleInput');
const autoStart       = document.getElementById('autoStartInput');
const soundPath       = document.getElementById('soundPath');
const askSoundPath    = document.getElementById('askSoundPath');
const ntfyTopic       = document.getElementById('ntfyTopic');
const browseBtn       = document.getElementById('browseBtn');
const browseAskBtn    = document.getElementById('browseAskBtn');
const testSoundBtn    = document.getElementById('testSoundBtn');
const testAskSoundBtn = document.getElementById('testAskSoundBtn');
const testNtfyBtn     = document.getElementById('testNtfyBtn');
const saveBtn         = document.getElementById('saveBtn');
const statusBar       = document.getElementById('statusBar');
const statusTxt       = document.getElementById('statusText');
const statusIco       = document.getElementById('statusIcon');
const statusDot       = document.getElementById('statusDot');

let statusTimer = null;

function syncDot(enabled) {
  statusDot.classList.toggle('off', !enabled);
}

async function loadConfig() {
  const cfg = await invoke('get_config');
  toggle.checked      = cfg.enabled;
  autoStart.checked   = cfg.auto_start;
  soundPath.value     = cfg.sound_path;
  askSoundPath.value  = cfg.ask_sound_path || '';
  ntfyTopic.value     = cfg.ntfy_topic || '';
  syncDot(cfg.enabled);
}

window.addEventListener('DOMContentLoaded', loadConfig);

// Reload config every time the window becomes visible (after being hidden to tray)
document.addEventListener('visibilitychange', () => {
  if (document.visibilityState === 'visible') {
    loadConfig();
  }
});

toggle.addEventListener('change', () => syncDot(toggle.checked));

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

testNtfyBtn.addEventListener('click', async () => {
  const topic = ntfyTopic.value.trim();
  if (!topic) {
    showStatus('Enter an ntfy topic first', 'err', xIcon());
    return;
  }
  testNtfyBtn.disabled = true;
  showStatus('Sending test notification...', 'info', spinnerIcon());
  const res = await invoke('test_ntfy', { topic });
  testNtfyBtn.disabled = false;
  if (res.ok) {
    showStatus('Notification sent to phone', 'ok', checkIcon());
  } else {
    showStatus('Failed — check your topic', 'err', xIcon());
  }
});

saveBtn.addEventListener('click', async () => {
  saveBtn.disabled = true;
  saveBtn.textContent = 'Saving...';
  const res = await invoke('save_config', {
    args: {
      enabled:        toggle.checked,
      auto_start:     autoStart.checked,
      sound_path:     soundPath.value,
      ask_sound_path: askSoundPath.value,
      ntfy_topic:     ntfyTopic.value.trim(),
    }
  });
  saveBtn.disabled = false;
  saveBtn.textContent = 'Save Settings';
  if (res.ok) {
    showStatus('Saved — restart Claude Code to apply', 'ok', checkIcon());
  } else {
    showStatus('Failed to save', 'err', xIcon());
  }
});

function showStatus(msg, type, iconHtml) {
  statusTxt.textContent = msg;
  statusIco.innerHTML   = iconHtml;
  statusBar.className   = `status-bar visible ${type}`;
  clearTimeout(statusTimer);
  statusTimer = setTimeout(() => statusBar.classList.remove('visible'), 4000);
}

function checkIcon()   { return '<polyline points="20 6 9 17 4 12"/>'; }
function xIcon()       { return '<line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>'; }
function spinnerIcon() { return '<path d="M12 2v4M12 18v4M4.93 4.93l2.83 2.83M16.24 16.24l2.83 2.83M2 12h4M18 12h4M4.93 19.07l2.83-2.83M16.24 7.76l2.83-2.83"/>'; }
