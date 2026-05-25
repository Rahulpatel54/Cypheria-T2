'use strict';

import { state } from './state.js';
import { vaultCall, rawInvoke } from './bridge.js';
import { showToast, closeModal } from './utils.js';

export async function loadSettings() {
  try {
    const s = await vaultCall('get_settings');
    if (!s) return;
    state.clipSecs = s.clear_clipboard_secs ?? 30;
    const el = id => document.getElementById(id);
    if (el('set-clipboard')) el('set-clipboard').value = String(state.clipSecs);
    if (el('set-startup') && s.launch_at_startup !== undefined) el('set-startup').checked = s.launch_at_startup;
    if (el('set-tray') && s.minimize_to_tray !== undefined) el('set-tray').checked = s.minimize_to_tray;
    if (el('set-showpwd') && s.show_password_default !== undefined) el('set-showpwd').checked = s.show_password_default;
    if (el('set-autolock') && s.auto_lock_secs !== undefined) el('set-autolock').value = String(s.auto_lock_secs);
  } catch (_) { }
}

export async function saveSettings() {
  if (state.settingsDebounce) clearTimeout(state.settingsDebounce);
  state.settingsDebounce = setTimeout(async () => {
    try {
      const settings = {
        theme: 'dark',
        launch_at_startup: document.getElementById('set-startup')?.checked ?? false,
        minimize_to_tray: document.getElementById('set-tray')?.checked ?? true,
        auto_lock_secs: parseInt(document.getElementById('set-autolock')?.value ?? '300'),
        show_password_default: document.getElementById('set-showpwd')?.checked ?? false,
        clear_clipboard_secs: parseInt(document.getElementById('set-clipboard')?.value ?? '30'),
      };
      state.clipSecs = settings.clear_clipboard_secs;
      await vaultCall('save_settings', { settings });
    } catch (_) { showToast('Failed to save settings', 'error'); }
  }, 600);
}

export async function changeMasterPassword() {
  const cur = document.getElementById('chpwd-current').value;
  const nw = document.getElementById('chpwd-new').value;
  const cf = document.getElementById('chpwd-confirm').value;
  const err = document.getElementById('chpwd-error');
  if (!err) return;
  err.textContent = '';

  if (!cur) { err.textContent = 'Enter your current password'; return; }
  if (nw.length < 8) { err.textContent = 'New password must be at least 8 characters'; return; }
  if (nw !== cf) {
    document.getElementById('chpwd-current').value = '';
    document.getElementById('chpwd-new').value = '';
    document.getElementById('chpwd-confirm').value = '';
    err.textContent = 'Passwords do not match';
    return;
  }

  document.getElementById('chpwd-current').value = '';
  document.getElementById('chpwd-new').value = '';
  document.getElementById('chpwd-confirm').value = '';

  const btn = document.getElementById('btn-chpwd-save'); if (btn) btn.disabled = true;
  try {
    await vaultCall('change_master_password', { oldPassword: cur, newPassword: nw });
    closeModal('modal-chpwd');
    showToast('Master password changed successfully', 'success');
  } catch (e) {
    const msg = String(e);
    err.textContent = msg.includes('AuthFailed') || msg.includes('Authentication failed')
      ? 'Current password is incorrect'
      : msg.replace(/^Error: /, '').slice(0, 120);
  } finally { if (btn) btn.disabled = false; }
}

export async function exportVault() {
  if (!state._invoke) { showToast('Export requires Tauri backend', 'warning'); return; }
  try {
    const dest = await rawInvoke('plugin:dialog|save', { title: 'Export Vault Backup', defaultPath: 'cypheria-backup.qvault', filters: [{ name: 'Cypheria Vault', extensions: ['qvault'] }] });
    if (!dest) return;
    await vaultCall('export_vault', { destinationPath: dest });
    showToast('Vault exported successfully', 'success');
  } catch (e) { showToast('Export failed: ' + String(e).slice(0, 80), 'error'); }
}

export function switchSettingsTab(tab) {
  document.querySelectorAll('.settings-nav .nav-item').forEach(n => n.classList.remove('active'));
  document.querySelector(`.settings-nav .nav-item[data-stab="${tab}"]`)?.classList.add('active');
  ['general', 'security', 'about'].forEach(t => {
    const el = document.getElementById('stab-' + t);
    if (el) el.style.display = t === tab ? '' : 'none';
  });
}
