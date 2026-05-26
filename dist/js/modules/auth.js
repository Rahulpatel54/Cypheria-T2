'use strict';

import { state } from './state.js';
import { rawInvoke, persistVaultPath, clearPersistedVaultPath } from './bridge.js';
import { showLoading, hideLoading, navigate } from './ui.js';
import { showToast } from './utils.js';

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
  // Clear the sidebar countdown timer when locking
  import('./ui.js').then(m => { m.clearAutolockCountdown(); }).catch(() => {});
  state.appUnlocked = false;

  state.vaultEntries = [];
   // Hide vault badge and clear name on lock
  const dashVaultName = document.getElementById('dash-vault-name');
  const dashVaultBadge = document.getElementById('dash-vault-badge');
  if (dashVaultName) dashVaultName.textContent = '';
  if (dashVaultBadge) dashVaultBadge.style.display = 'none';
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

  const badge = document.getElementById('vault-name-badge');
  if (badge) badge.style.display = 'none';
  state.currentVaultName = null;
  const unlockBtn = document.getElementById('unlock-btn');
  if (unlockBtn) { unlockBtn.disabled = false; unlockBtn.textContent = 'Unlock'; }
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
  // Reset button text on every exit path so it never stays "Unlocking…"
  } catch (e) {
    pwd = null;
    state.lockAttempts++;
    const msg = String(e);
    if (msg.includes('RateLimited') || msg.includes('Try again in')) {
      const m = msg.match(/(\d+)/);
      startLockCooldown(m ? parseInt(m[1]) : 30);
      btn.disabled = false;
      btn.textContent = 'Unlock';
      return;
    }
    if (state.lockAttempts >= 5) {
      startLockCooldown(30);
      btn.disabled = false;
      btn.textContent = 'Unlock';
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
  // Stop the sidebar countdown timer immediately on manual lock
  import('./ui.js').then(m => m.clearAutolockCountdown()).catch(() => {});
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

// Show a styled choice modal instead of native dialog for vault switching
export async function openDifferentVault() {
  if (!state._invoke) { showToast('File dialog requires Tauri backend', 'warning'); return; }

  // Build an inline choice overlay
  const overlay = document.createElement('div');
  overlay.style.cssText = 'position:fixed;inset:0;background:rgba(0,0,0,0.7);backdrop-filter:blur(4px);display:flex;align-items:center;justify-content:center;z-index:9999;';

  const card = document.createElement('div');
  card.style.cssText = 'background:var(--bg-modal);border:1px solid var(--border-accent);border-radius:var(--radius-lg);padding:32px;width:380px;text-align:center;box-shadow:var(--glow-purple-md);';

  card.innerHTML = `
    <div style="font-family:'Syne',sans-serif;font-size:18px;font-weight:700;margin-bottom:8px;color:var(--text-primary);">Switch Vault</div>
    <p style="font-size:13px;color:var(--text-muted);margin-bottom:24px;line-height:1.6;">Open an existing vault file or start fresh with a new one.</p>
    <div style="display:flex;flex-direction:column;gap:10px;">
      <button id="_vswitch_open" style="width:100%;padding:12px 16px;background:var(--accent);color:#fff;border:none;border-radius:var(--radius-sm);font-size:14px;font-weight:600;cursor:pointer;font-family:'DM Sans',sans-serif;display:flex;align-items:center;gap:10px;justify-content:center;">
        <svg viewBox="0 0 24 24" width="16" height="16" style="stroke:currentColor;fill:none;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;flex-shrink:0;"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>
        Open Existing Vault
      </button>
      <button id="_vswitch_new" style="width:100%;padding:12px 16px;background:var(--bg-card);color:var(--text-primary);border:1px solid var(--border-mid);border-radius:var(--radius-sm);font-size:14px;font-weight:600;cursor:pointer;font-family:'DM Sans',sans-serif;display:flex;align-items:center;gap:10px;justify-content:center;">
        <svg viewBox="0 0 24 24" width="16" height="16" style="stroke:currentColor;fill:none;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;flex-shrink:0;"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Create New Vault
      </button>
      <button id="_vswitch_cancel" style="width:100%;padding:9px 16px;background:none;color:var(--text-muted);border:none;border-radius:var(--radius-sm);font-size:13px;cursor:pointer;font-family:'DM Sans',sans-serif;">Cancel</button>
    </div>`;

  overlay.appendChild(card);
  document.body.appendChild(overlay);

  const cleanup = () => document.body.removeChild(overlay);

  document.getElementById('_vswitch_cancel').onclick = cleanup;
  overlay.onclick = e => { if (e.target === overlay) cleanup(); };

  document.getElementById('_vswitch_open').onclick = async () => {
    cleanup();
    try {
      const p = await rawInvoke('plugin:dialog|open', {
        options: { title: 'Open Vault', multiple: false, directory: false, filters: [{ name: 'Cypheria Vault', extensions: ['qvault'] }] },
      });
      if (!p) return;
      const path = typeof p === 'string' ? p : p[0];
      const canonical = await rawInvoke('open_vault', { vaultPath: path });
      state.currentVaultPath = canonical;
      const { persistVaultPath } = await import('./bridge.js');
      await persistVaultPath(canonical);
        try {
          const meta = await rawInvoke('get_vault_meta', { vaultPath: canonical });
          const name = meta?.vault_name
            || canonical.split(/[\\/]/).pop().replace(/\.qvault$/i, '');
          document.getElementById('lock-vault-name').textContent = name;
          state.currentVaultName = name;
        } catch (_) {
          const name = canonical.split(/[\\/]/).pop().replace(/\.qvault$/i, '');
          document.getElementById('lock-vault-name').textContent = name;
          state.currentVaultName = name;
        }
      document.getElementById('lock-error').textContent = '';
      showToast('Vault loaded — enter your password', 'success');
    } catch (e) { showToast('Could not open vault: ' + String(e).slice(0, 80), 'error'); }
  };

  document.getElementById('_vswitch_new').onclick = async () => {
    cleanup();
    const { clearPersistedVaultPath } = await import('./bridge.js');
    await clearPersistedVaultPath();
    state.currentVaultPath = null;
    showSetupScreen();
  };
}

export async function afterUnlock() {
  showLoading('Loading vault…');
  document.getElementById('lock-screen').classList.add('hidden');
  document.getElementById('setup-screen').classList.add('hidden');
  document.getElementById('app').classList.add('visible');
  navigate('dashboard');

  try {
    const { loadEntries } = await import('./vault.js');
    const { loadNotes }   = await import('./notes.js');
    const { loadSettings } = await import('./settings.js');
    await Promise.all([loadEntries(), loadNotes(), loadSettings()]);
    try {
      const meta = await rawInvoke('get_vault_meta', { vaultPath: state.currentVaultPath });
      const name = meta.vault_name || state.currentVaultPath?.split(/[\\/]/).pop().replace('.qvault','') || '';
      const nameEl = document.getElementById('dash-vault-name');
      const badge = document.getElementById('dash-vault-badge');
      if (nameEl) nameEl.textContent = name;
      if (badge && name) badge.style.display = 'inline-flex';
    } catch (_) {}
    import('./vault.js').then(m => m.loadPasswordScores()).catch(() => {});
  } catch (e) {
    console.warn('[Cypheria] Partial load after unlock:', e);
  }

  // Resolve and store vault name, then show the titlebar badge
  try {
    if (state.currentVaultPath) {
      const meta = await rawInvoke('get_vault_meta', { vaultPath: state.currentVaultPath });
      state.currentVaultName = meta?.vault_name
        || state.currentVaultPath.split(/[\\/]/).pop().replace(/\.qvault$/i, '');
    }
  } catch (_) {
    // Fall back to filename if meta read fails
    state.currentVaultName = state.currentVaultPath
      ? state.currentVaultPath.split(/[\\/]/).pop().replace(/\.qvault$/i, '')
      : 'Vault';
  }

  const badge     = document.getElementById('vault-name-badge');
  const badgeText = document.getElementById('vault-name-badge-text');
  if (badge && badgeText && state.currentVaultName) {
    badgeText.textContent = state.currentVaultName;
    badge.style.display   = 'flex';
  }

  hideLoading();

  // Start autolock countdown using persisted timeout from settings
  try {
    const actualTimeout = parseInt(document.getElementById('set-autolock')?.value || '300');
    const { startAutolockCountdown } = await import('./ui.js');
    startAutolockCountdown(actualTimeout);
  } catch (_) {}
}
