'use strict';

import { state } from './state.js';
import { rawInvoke, persistVaultPath, clearPersistedVaultPath } from './bridge.js';
import { showToast, closeModal, openModal, toggleEye, pwdStrength, makeAvatar } from './utils.js';
import { 
  renderVaultTable, renderDashboard, renderFavorites, 
  loadEntries, openAddModal, saveNewEntry, saveEditEntry,
  selectEntry
} from './vault.js';
import { loadNotes, renderNotes, openNoteModal, saveNote } from './notes.js';
import { loadPasswordScores } from './vault.js';
import { generatePassword, copGenPwd } from './generator.js';
import { 
  showSetupScreen, gotoSetupStep, browseSavePath, createVault, 
  tryUnlock, openDifferentVault, lockVaultUI 
} from './auth.js';
import { saveSettings, changeMasterPassword, exportVault, switchSettingsTab } from './settings.js';

export function showLoading(msg = 'Loading…') {
  const el = document.getElementById('loading-msg');
  if (el) el.textContent = msg;
  const overlay = document.getElementById('loading-overlay');
  if (overlay) overlay.classList.remove('hidden');
}

export function hideLoading() {
  const overlay = document.getElementById('loading-overlay');
  if (overlay) overlay.classList.add('hidden');
}

export function navigate(page) {
  const res = document.getElementById('search-results');
  if (res) res.classList.remove('open');
  const input = document.getElementById('search-input');
  if (input) input.value = '';

  document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
  document.querySelectorAll('#sidebar .nav-item[data-page]').forEach(n => n.classList.remove('active'));
  const pg = document.getElementById('page-' + page);
  if (pg) pg.classList.add('active');
  const nav = document.querySelector(`#sidebar .nav-item[data-page="${page}"]`);
  if (nav) nav.classList.add('active');
  if (page === 'vault') renderVaultTable();
  if (page === 'favorites') renderFavorites();
  if (page === 'notes') renderNotes();
  if (page === 'dashboard') renderDashboard();
  if (page === 'dashboard' && state.vaultEntries.length > 0 && !Object.keys(state.passwordScores).length) {
    setTimeout(loadPasswordScores, 120);
  }
  if (page === 'generator') {
    generatePassword();
    setTimeout(() => {
      const list = document.getElementById('pp-list');
      if (list && !list.innerHTML.trim()) {
        document.dispatchEvent(new CustomEvent('sp:navigateToGenerator'));
      }
    }, 80);
  }
}

export function handleSearch(val) {
  const res = document.getElementById('search-results');
  if (!res) return;
  if (!val.trim()) { res.classList.remove('open'); return; }
  const matches = state.vaultEntries.filter(e => {
    const v = val.toLowerCase();
    return (e.name || '').toLowerCase().includes(v) || (e.username || '').toLowerCase().includes(v) || (e.website || '').toLowerCase().includes(v);
  }).slice(0, 8);
  res.innerHTML = '';
  if (!matches.length) {
    const d = document.createElement('div'); d.style.cssText = 'padding:14px 16px;color:var(--text-muted);font-size:13px;'; d.textContent = 'No results found'; res.appendChild(d);
  } else {
    matches.forEach(e => {
      const item = document.createElement('div'); item.className = 'search-result-item';
      const info = document.createElement('div');
      const n = document.createElement('div'); n.style.cssText = 'font-size:13px;font-weight:500;'; n.textContent = e.name;
      const u = document.createElement('div'); u.style.cssText = 'font-size:11px;color:var(--text-muted);'; u.textContent = e.username || '';
      info.appendChild(n); info.appendChild(u);
      item.appendChild(makeAvatar(e, 24)); item.appendChild(info);
      item.onclick = () => { res.classList.remove('open'); document.getElementById('search-input').value = ''; navigate('vault'); setTimeout(() => selectEntry(e.id), 60); };
      res.appendChild(item);
    });
  }
  res.classList.add('open');
}

export function startClipCountdown() {
  if (state.clipTimer) { clearTimeout(state.clipTimer); state.clipTimer = null; }
  if (state.clipInterval) { clearInterval(state.clipInterval); state.clipInterval = null; }

  const ind = document.getElementById('clip-indicator');
  const cnt = document.getElementById('clip-countdown');
  if (!ind || !cnt) return;
  const s = state.clipSecs || 30;
  let rem = s;
  cnt.textContent = rem;
  ind.style.display = 'block';

  state.clipInterval = setInterval(() => {
    rem--;
    cnt.textContent = rem;
    if (rem <= 0) {
      clearInterval(state.clipInterval);
      state.clipInterval = null;
      ind.style.display = 'none';
    }
  }, 1000);

  state.clipTimer = setTimeout(() => {
    if (state._invoke) rawInvoke('clear_clipboard').catch(() => { });
    ind.style.display = 'none';
    clearInterval(state.clipInterval);
    state.clipInterval = null;
    state.clipTimer = null;
  }, s * 1000);
}

export function updateAddStrength() {
  const pwd = document.getElementById('add-password').value;
  const { score, label, color } = pwdStrength(pwd);
  const bar = document.getElementById('add-strength-bar');
  if (bar) bar.style.cssText = `width:${score}%;background:${color};`;
  const lbl = document.getElementById('add-strength-label');
  if (lbl) {
    lbl.style.color = color;
    lbl.textContent = label;
  }
}

export function updateSetupStrength() {
  const pwd = document.getElementById('setup-pwd').value;
  const { score, label, color } = pwdStrength(pwd);
  const bar = document.getElementById('setup-strength-bar');
  if (bar) bar.style.cssText = `width:${score}%;background:${color};`;
  const lbl = document.getElementById('setup-strength-label');
  if (lbl) {
    lbl.style.color = color;
    lbl.textContent = label || 'Enter a password';
  }
}

export function wireEvents() {
  // Setup step navigation
  document.getElementById('btn-get-started')?.addEventListener('click', () => gotoSetupStep(2));
  document.getElementById('btn-open-existing')?.addEventListener('click', () => {
    if (!state._invoke) { showToast('Requires Tauri backend', 'warning'); return; }
    rawInvoke('plugin:dialog|open', {
      options: {
        title: 'Open Vault',
        multiple: false,
        directory: false,
        filters: [{ name: 'Cypheria Vault', extensions: ['qvault'] }],
      },
    })
      .then(async p => {
        if (!p) return;
        const path = typeof p === 'string' ? p : p[0];
        const canonical = await rawInvoke('open_vault', { vaultPath: path });
        state.currentVaultPath = canonical;
        await persistVaultPath(canonical);
        document.getElementById('setup-screen').classList.add('hidden');
        try {
          const meta = await rawInvoke('get_vault_meta', { vaultPath: canonical });
          document.getElementById('lock-vault-name').textContent = meta.vault_name || canonical.split(/[\\/]/).pop().replace('.qvault', '');
        } catch (_) {
          document.getElementById('lock-vault-name').textContent = canonical.split(/[\\/]/).pop().replace('.qvault', '');
        }
        document.getElementById('lock-error').textContent = '';
        const { showLockScreen } = await import('./auth.js');
        showLockScreen();
      })
      .catch(e => showToast('Could not open vault: ' + String(e).slice(0, 80), 'error'));
  });
  document.getElementById('btn-browse')?.addEventListener('click', browseSavePath);
  document.getElementById('btn-step2-back')?.addEventListener('click', () => gotoSetupStep(1));
  document.getElementById('btn-step2-next')?.addEventListener('click', () => {
    const name = document.getElementById('setup-name').value.trim();
    const path = document.getElementById('setup-path').value.trim();
    if (!name) { showToast('Please enter a vault name', 'warning'); return; }
    if (!path) { showToast('Please choose a save location', 'warning'); return; }
    gotoSetupStep(3);
  });
  document.getElementById('btn-step3-back')?.addEventListener('click', () => gotoSetupStep(2));
  document.getElementById('btn-create')?.addEventListener('click', createVault);
  document.getElementById('setup-pwd')?.addEventListener('input', updateSetupStrength);
  document.getElementById('setup-eye1')?.addEventListener('click', () => toggleEye('setup-pwd'));
  document.getElementById('setup-eye2')?.addEventListener('click', () => toggleEye('setup-confirm'));

  // Lock screen
  document.getElementById('unlock-btn')?.addEventListener('click', tryUnlock);
  document.getElementById('master-pwd')?.addEventListener('keydown', e => { if (e.key === 'Enter') tryUnlock(); });
  document.getElementById('lock-eye-btn')?.addEventListener('click', () => toggleEye('master-pwd'));
  document.getElementById('lock-alt')?.addEventListener('click', openDifferentVault);

  // Titlebar
  document.getElementById('logo-link')?.addEventListener('click', () => navigate('dashboard'));
  document.getElementById('tb-settings')?.addEventListener('click', () => navigate('settings'));
  document.getElementById('tb-lock')?.addEventListener('click', lockVaultUI);
  document.getElementById('win-close')?.addEventListener('click', () =>
    rawInvoke('plugin:window|close', { label: 'main' }).catch(() => { }));
  document.getElementById('win-min')?.addEventListener('click', () =>
    rawInvoke('plugin:window|minimize', { label: 'main' }).catch(() => { }));
  document.getElementById('win-max')?.addEventListener('click', () =>
    rawInvoke('plugin:window|internal_toggle_maximize', { label: 'main' }).catch(() => { }));

  // Sidebar nav
  document.querySelectorAll('#sidebar .nav-item[data-page]').forEach(n => {
    n.addEventListener('click', () => navigate(n.dataset.page));
  });
  document.getElementById('nav-lock')?.addEventListener('click', lockVaultUI);

  // Dashboard
  document.getElementById('dash-add-entry')?.addEventListener('click', openAddModal);
  document.querySelectorAll('[data-page]').forEach(el => {
    if (!el.closest('#sidebar')) el.addEventListener('click', () => navigate(el.dataset.page));
  });

  // Vault
  document.getElementById('vault-add-entry')?.addEventListener('click', openAddModal);
  document.getElementById('vault-refresh')?.addEventListener('click', loadEntries);
  document.getElementById('security-refresh-btn')?.addEventListener('click', () => {
    state.passwordScores = {};
    import('./vault.js').then(m => m.loadPasswordScores()).catch(() => {});
  });
  document.getElementById('vault-sort')?.addEventListener('change', renderVaultTable);

  // Favorites
  document.getElementById('fav-sort')?.addEventListener('change', renderFavorites);

  // Notes
  document.getElementById('note-new-btn')?.addEventListener('click', () => openNoteModal());
  document.getElementById('btn-note-save')?.addEventListener('click', saveNote);

  // Generator
  document.getElementById('gen-btn')?.addEventListener('click', generatePassword);
  document.getElementById('gen-copy')?.addEventListener('click', copGenPwd);
  document.getElementById('gen-copy2')?.addEventListener('click', copGenPwd);
  ['gen-length', 'gen-upper', 'gen-lower', 'gen-numbers', 'gen-symbols'].forEach(id => {
    document.getElementById(id)?.addEventListener('change', generatePassword);
  });

  // Add entry modal
  document.getElementById('btn-add-save')?.addEventListener('click', saveNewEntry);
  document.getElementById('add-password')?.addEventListener('input', updateAddStrength);
  document.getElementById('add-pwd-eye')?.addEventListener('click', () => toggleEye('add-password'));
  document.getElementById('add-pwd-gen')?.addEventListener('click', () => {
    const p = document.getElementById('gen-output').textContent;
    if (p && p !== 'Click Generate') {
      document.getElementById('add-password').value = p;
      updateAddStrength();
    } else {
      generatePassword();
      setTimeout(() => {
        const p2 = document.getElementById('gen-output').textContent;
        if (p2 && p2 !== 'Click Generate') { document.getElementById('add-password').value = p2; updateAddStrength(); }
      }, 300);
    }
  });

  // Edit entry modal
  document.getElementById('btn-edit-save')?.addEventListener('click', saveEditEntry);
  document.getElementById('edit-pwd-eye')?.addEventListener('click', () => toggleEye('edit-password'));

  // Change password modal
  document.getElementById('btn-chpwd')?.addEventListener('click', () => openModal('modal-chpwd'));
  document.getElementById('btn-chpwd-save')?.addEventListener('click', changeMasterPassword);

  // Export
  document.getElementById('btn-export')?.addEventListener('click', exportVault);

  // Confirm modal
document.getElementById('btn-confirm-action')?.addEventListener('click', async () => {
    if (state.confirmCallback) {
      const btn = document.getElementById('btn-confirm-action');
      btn.disabled = true;
      try {
        await state.confirmCallback();
        closeModal('modal-confirm');
        showToast('Deleted successfully', 'success');
      } catch (e) {
        closeModal('modal-confirm');
        showToast('Delete failed: ' + String(e).slice(0, 80), 'error');
      } finally {
        btn.disabled = false;
        state.confirmCallback = null;
      }
    }
  });

  // Close buttons on modals
  document.querySelectorAll('[data-close]').forEach(el => {
    el.addEventListener('click', () => {
      closeModal(el.dataset.close);
      if (el.dataset.close === 'modal-edit-entry') {
        document.getElementById('edit-password').value = '';
        document.getElementById('edit-id').value = '';
      }
      if (el.dataset.close === 'modal-chpwd') {
        document.getElementById('chpwd-current').value = '';
        document.getElementById('chpwd-new').value = '';
        document.getElementById('chpwd-confirm').value = '';
      }
    });
  });

  // Settings tabs
  document.querySelectorAll('.settings-nav .nav-item[data-stab]').forEach(n => {
    n.addEventListener('click', () => switchSettingsTab(n.dataset.stab));
  });

  // Settings changes
  ['set-startup', 'set-tray', 'set-autolock', 'set-clipboard', 'set-showpwd'].forEach(id => {
    document.getElementById(id)?.addEventListener('change', saveSettings);
  });

  // Search
  document.getElementById('search-input')?.addEventListener('input', e => handleSearch(e.target.value));
  document.getElementById('search-input')?.addEventListener('keydown', e => {
    if (e.key === 'Escape') { document.getElementById('search-results').classList.remove('open'); e.target.value = ''; }
  });

  // Ctrl+K
  document.addEventListener('keydown', e => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'k') {
      e.preventDefault();
      document.getElementById('search-input').focus();
    }
    if (e.key === 'Escape') {
      const res = document.getElementById('search-results');
      const searchOpen = res && res.classList.contains('open');
      if (searchOpen) {
        res.classList.remove('open');
        document.getElementById('search-input').value = '';
        return;
      }
      document.querySelectorAll('.modal-overlay.open').forEach(m => m.classList.remove('open'));
    }
  });

  document.addEventListener('click', e => {
    const wrap = document.getElementById('title-search-wrap');
    const res = document.getElementById('search-results');
    if (wrap && res && !e.target.closest('#title-search-wrap') && !e.target.closest('#search-results')) {
      res.classList.remove('open');
    }
  });

  if (!window.__TAURI_INTERNALS__?.isTauriDevTools) {
    document.addEventListener('contextmenu', e => e.preventDefault());
  }
}
