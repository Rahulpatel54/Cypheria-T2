'use strict';

import { state } from './state.js';
import { rawInvoke, persistVaultPath, clearPersistedVaultPath } from './bridge.js';
import { showToast, showLoading, hideLoading, navigate } from './ui.js';

export let setupStep = 0;

export function showSetupScreen() {
  document.getElementById('lock-screen').classList.add('hidden');
  document.getElementById('setup-screen').classList.remove('hidden');
  document.getElementById('app').classList.remove('visible');
  gotoSetupStep(1);
}

export function gotoSetupStep(n) {
  setupStep = n;
  [1, 2, 3].forEach(i => {
    const el = document.getElementById('step-' + i);
    if (el) el.style.display = i === n ? '' : 'none';
  });
  if (n === 1) {
    document.getElementById('setup-name').value = 'My Vault';
    document.getElementById('setup-path').value = '';
    document.getElementById('setup-pwd').value = '';
    document.getElementById('setup-confirm').value = '';
    clearSetupError();
  }
  if (n === 2) {
    suggestPath();
  }
  if (n === 3) {
    setTimeout(() => document.getElementById('setup-pwd').focus(), 100);
  }
}

function clearSetupError() {
  const e = document.getElementById('setup-error');
  e.textContent = '';
  e.style.display = 'none';
}

function showSetupError(msg) {
  const e = document.getElementById('setup-error');
  e.textContent = msg;
  e.style.display = 'block';
}

export async function suggestPath() {
  if (!state._invoke) return;
  const nameRaw = document.getElementById('setup-name').value.trim() || 'MyVault';
  const name = nameRaw.replace(/[^a-zA-Z0-9_\-]/g, '_');
  const pathEl = document.getElementById('setup-path');
  const hint = document.getElementById('path-hint');

  if (pathEl.value) return;

  try {
    let base = null;
    for (const dir of [6, 4, 9]) {
      try {
        base = await rawInvoke('plugin:path|resolve_directory', { directory: dir });
        if (base && typeof base === 'string') break;
      } catch (_) { }
    }

    if (pathEl.value) return;

    if (base && typeof base === 'string') {
      const sep = base.includes('/') ? '/' : '\\';
      const proposed = `${base}${sep}Cypheria${sep}${name}.qvault`;
      pathEl.value = proposed;
      hint.textContent = '✓ Default: Documents/Cypheria — click Browse to change';
      hint.style.color = 'var(--color-green)';
    } else {
      throw new Error('no base dir resolved');
    }
  } catch (_) {
    if (pathEl.value) return;
    pathEl.value = '';
    hint.textContent = 'Could not detect your Documents folder. Please click Browse to choose a save location.';
    hint.style.color = 'var(--color-amber)';
  }
}

export async function browseSavePath() {
  if (!state._invoke) { showToast('File dialog requires Tauri backend', 'warning'); return; }
  try {
    const name = (document.getElementById('setup-name').value.trim() || 'MyVault')
      .replace(/[^a-zA-Z0-9_\-]/g, '_');

    const path = await rawInvoke('plugin:dialog|save', {
      options: {
        title: 'Save Vault As',
        defaultPath: name + '.qvault',
        filters: [{ name: 'Cypheria Vault', extensions: ['qvault'] }],
      },
    });

    if (path && typeof path === 'string') {
      const finalPath = path.endsWith('.qvault') ? path : path + '.qvault';
      document.getElementById('setup-path').value = finalPath;
      document.getElementById('path-hint').textContent = '✓ Path selected';
      document.getElementById('path-hint').style.color = 'var(--color-green)';
    }
  } catch (e) {
    console.error('[Cypheria] browseSavePath failed:', e);
    showToast('Could not open file dialog: ' + String(e).slice(0, 120), 'error');
  }
}

export async function createVault() {
  const name = document.getElementById('setup-name').value.trim();
  const path = document.getElementById('setup-path').value.trim();
  const pwd = document.getElementById('setup-pwd').value;
  const confirm = document.getElementById('setup-confirm').value;

  clearSetupError();

  if (!name) { showSetupError('Vault name cannot be empty'); return; }
  if (!path) { showSetupError('Please choose a save location'); return; }
  if (pwd.length < 8) { showSetupError('Password must be at least 8 characters'); return; }
  if (pwd !== confirm) {
    document.getElementById('setup-pwd').value = '';
    document.getElementById('setup-confirm').value = '';
    showSetupError('Passwords do not match');
    return;
  }

  const btn = document.getElementById('btn-create');
  btn.disabled = true;
  btn.textContent = 'Creating vault…';
  showLoading('Creating vault… (this may take a moment)');

  document.getElementById('setup-pwd').value = '';
  document.getElementById('setup-confirm').value = '';

  try {
    await rawInvoke('create_vault', { password: pwd, vaultPath: path, vaultName: name });
    await rawInvoke('unlock_vault', { password: pwd, vaultPath: path });

    state.currentVaultPath = path;
    await persistVaultPath(path);

    document.getElementById('setup-screen').classList.add('hidden');
    state.appUnlocked = true;
    await afterUnlock();
    showToast('Vault created successfully!', 'success');

  } catch (e) {
    hideLoading();
    const msg = String(e).replace(/^Error: /, '');
    showSetupError(msg.includes('VaultExists')
      ? 'A vault already exists at that path. Choose a different location.'
      : msg.slice(0, 150));
    btn.disabled = false;
    btn.textContent = 'Create Vault';
  }
}

export function showLockScreen() {
  state.appUnlocked = false;

  state.vaultEntries = [];
  state.vaultNotes = [];
  state.selectedEntryId = null;

  const tbody = document.getElementById('vault-tbody');
  if (tbody) tbody.innerHTML = '';
  const favTbody = document.getElementById('fav-tbody');
  if (favTbody) favTbody.innerHTML = '';
  const notesGrid = document.getElementById('notes-grid');
  if (notesGrid) notesGrid.innerHTML = '';
  const recentList = document.getElementById('recent-entries-list');
  if (recentList) recentList.innerHTML = '';
  const vaultDetail = document.getElementById('vault-detail');
  if (vaultDetail) vaultDetail.innerHTML = '';

  const dashTotal = document.getElementById('dash-total');
  if (dashTotal) dashTotal.textContent = '0';
  const dashFavs = document.getElementById('dash-favs');
  if (dashFavs) dashFavs.textContent = '0';
  const dashNotes = document.getElementById('dash-notes');
  if (dashNotes) dashNotes.textContent = '0';

  state.passwordRevealTimers.forEach(tid => clearTimeout(tid));
  state.passwordRevealTimers.clear();

  if (state.clipTimer) { clearTimeout(state.clipTimer); state.clipTimer = null; }
  if (state.clipInterval) { clearInterval(state.clipInterval); state.clipInterval = null; }
  const clipInd = document.getElementById('clip-indicator');
  if (clipInd) clipInd.style.display = 'none';

  if (state.settingsDebounce) { clearTimeout(state.settingsDebounce); state.settingsDebounce = null; }

  document.querySelectorAll('.modal-overlay.open').forEach(m => m.classList.remove('open'));

  ['add-password', 'edit-password', 'chpwd-current', 'chpwd-new', 'chpwd-confirm'].forEach(id => {
    const el = document.getElementById(id);
    if (el) el.value = '';
  });

  document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
  const dash = document.getElementById('page-dashboard');
  if (dash) dash.classList.add('active');
  document.querySelectorAll('#sidebar .nav-item[data-page]').forEach(n => n.classList.remove('active'));
  const dashNav = document.querySelector('#sidebar .nav-item[data-page="dashboard"]');
  if (dashNav) dashNav.classList.add('active');

  document.getElementById('setup-screen').classList.add('hidden');
  document.getElementById('lock-screen').classList.remove('hidden');
  document.getElementById('app').classList.remove('visible');

  document.getElementById('master-pwd').value = '';
  document.getElementById('lock-error').textContent = '';
  document.getElementById('lock-attempts').textContent = '';

  setTimeout(() => document.getElementById('master-pwd').focus(), 150);
}

export function hideLockScreen() {
  document.getElementById('lock-screen').classList.add('hidden');
  document.getElementById('app').classList.add('visible');
}

export async function tryUnlock() {
  if (state.lockCooldown) return;

  let pwd = document.getElementById('master-pwd').value;
  document.getElementById('master-pwd').value = '';

  if (!pwd) {
    document.getElementById('lock-error').textContent = 'Please enter your master password';
    return;
  }
  if (!state.currentVaultPath) {
    pwd = null;
    document.getElementById('lock-error').textContent = 'No vault loaded. Please open or create a vault.';
    return;
  }

  const btn = document.getElementById('unlock-btn');
  btn.disabled = true;
  btn.textContent = 'Unlocking…';
  document.getElementById('lock-error').textContent = '';

  try {
    await rawInvoke('unlock_vault', { password: pwd, vaultPath: state.currentVaultPath });
    pwd = null;
    state.lockAttempts = 0;
    state.appUnlocked = true;
    hideLockScreen();
    await afterUnlock();
    showToast('Vault unlocked', 'success');
  } catch (e) {
    pwd = null;
    state.lockAttempts++;
    const msg = String(e);
    if (msg.includes('RateLimited') || msg.includes('Try again in')) {
      const m = msg.match(/(\d+)/);
      startLockCooldown(m ? parseInt(m[1]) : 30);
      return;
    }
    if (state.lockAttempts >= 5) {
      startLockCooldown(30);
      return;
    }
    const left = 5 - state.lockAttempts;
    document.getElementById('lock-error').textContent =
      msg.includes('AuthFailed') || msg.includes('Authentication failed')
        ? `Incorrect password. ${left} attempt${left !== 1 ? 's' : ''} remaining.`
        : msg.replace(/^Error: /, '').slice(0, 120);
    document.getElementById('lock-attempts').textContent = `${state.lockAttempts}/5 attempts`;
    btn.disabled = false;
    btn.textContent = 'Unlock';
  }
}

function startLockCooldown(seconds) {
  state.lockCooldown = true; state.lockAttempts = 0;
  const btn = document.getElementById('unlock-btn');
  document.getElementById('lock-error').textContent = 'Too many failed attempts. Please wait.';
  let s = seconds;
  btn.disabled = true; btn.textContent = `Wait ${s}s`;
  const t = setInterval(() => {
    s--;
    btn.textContent = `Wait ${s}s`;
    if (s <= 0) {
      clearInterval(t);
      btn.disabled = false; btn.textContent = 'Unlock';
      state.lockCooldown = false;
      document.getElementById('lock-error').textContent = '';
      document.getElementById('lock-attempts').textContent = '';
    }
  }, 1000);
}

export async function lockVaultUI() {
  if (state.clipTimer) { clearTimeout(state.clipTimer); state.clipTimer = null; }
  if (state.clipInterval) { clearInterval(state.clipInterval); state.clipInterval = null; }
  const clipInd = document.getElementById('clip-indicator');
  if (clipInd) clipInd.style.display = 'none';

  if (state._invoke) await rawInvoke('clear_clipboard').catch(() => { });

  try { await rawInvoke('lock_vault'); } catch (_) { }
  state.appUnlocked = false;
  if (state.settingsDebounce) { clearTimeout(state.settingsDebounce); state.settingsDebounce = null; }
  state.passwordRevealTimers.forEach(tid => clearTimeout(tid));
  state.passwordRevealTimers.clear();
  showLockScreen();
}

export async function openDifferentVault() {
  if (!state._invoke) { showToast('File dialog requires Tauri backend', 'warning'); return; }
  let choice;
  try {
    choice = await rawInvoke('plugin:dialog|ask', {
      options: {
        title: 'Switch Vault',
        message: 'What would you like to do?',
        okLabel: 'Open Existing Vault',
        cancelLabel: 'Create New Vault',
      },
    });
  } catch (_) {
    choice = true;
  }

  if (choice) {
    try {
      const p = await rawInvoke('plugin:dialog|open', {
        options: {
          title: 'Open Vault',
          multiple: false,
          directory: false,
          filters: [{ name: 'Cypheria Vault', extensions: ['qvault'] }],
        },
      });
      if (p) {
        const path = typeof p === 'string' ? p : p[0];
        const canonical = await rawInvoke('open_vault', { vaultPath: path });
        state.currentVaultPath = canonical;
        await persistVaultPath(canonical);
        try {
          const meta = await rawInvoke('get_vault_meta', { vaultPath: canonical });
          document.getElementById('lock-vault-name').textContent =
            meta.vault_name || canonical.split(/[\\/]/).pop().replace('.qvault', '');
        } catch (_) {
          document.getElementById('lock-vault-name').textContent =
            canonical.split(/[\\/]/).pop().replace('.qvault', '');
        }
        document.getElementById('lock-error').textContent = '';
        showToast('Vault loaded — enter your password', 'success');
      }
    } catch (e) {
      console.error('[Cypheria] openDifferentVault failed:', e);
      showToast('Could not open vault: ' + String(e).slice(0, 80), 'error');
    }
  } else {
    await clearPersistedVaultPath();
    state.currentVaultPath = null;
    showSetupScreen();
  }
}

export async function afterUnlock() {
  showLoading('Loading vault…');
  document.getElementById('lock-screen').classList.add('hidden');
  document.getElementById('setup-screen').classList.add('hidden');
  document.getElementById('app').classList.add('visible');
  navigate('dashboard');
  try {
    const { loadEntries } = await import('./vault.js');
    const { loadNotes } = await import('./notes.js');
    const { loadSettings } = await import('./settings.js');
    await Promise.all([loadEntries(), loadNotes(), loadSettings()]);
  } catch (e) {
    console.warn('[Cypheria] Partial load after unlock:', e);
  }
  hideLoading();
}
