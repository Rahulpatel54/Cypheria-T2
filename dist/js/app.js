'use strict';

import { state } from './modules/state.js';
import { initTauri, rawInvoke, getPersistedVaultPath, persistVaultPath, clearPersistedVaultPath } from './modules/bridge.js';
import { showToast, showLoading, hideLoading, navigate, wireEvents } from './modules/ui.js';
import { showSetupScreen, showLockScreen } from './modules/auth.js';
import { initSmartPanel } from './modules/smart-panel.js';

async function setupTauriEvents() {
  try {
    const listen = window.__TAURI_INTERNALS__?.listen || window.__TAURI__?.event?.listen;
    if (listen) {
      await listen('vault-auto-locked', async () => {
        if (state._invoke) await rawInvoke('clear_clipboard').catch(() => { });
        showLockScreen();
        showToast('Vault locked due to inactivity', 'warning');
      });
      await listen('vault-locked', async () => {
        if (state._invoke) await rawInvoke('clear_clipboard').catch(() => { });
        showLockScreen();
      });
    }
  } catch (_) { }
}

async function checkInitialState() {
  if (!state._invoke) { showSetupScreen(); return; }

  const stored = await getPersistedVaultPath();
  if (!stored) { showSetupScreen(); return; }

  try {
    const canonical = await rawInvoke('open_vault', { vaultPath: stored });
    state.currentVaultPath = canonical;
    await persistVaultPath(canonical);
    try {
      const meta = await rawInvoke('get_vault_meta', { vaultPath: canonical });
      document.getElementById('lock-vault-name').textContent = meta.vault_name || canonical.split(/[\\/]/).pop().replace('.qvault', '');
    } catch (_) {
      document.getElementById('lock-vault-name').textContent = canonical.split(/[\\/]/).pop().replace('.qvault', '');
    }
    showLockScreen();
  } catch (_) {
    await clearPersistedVaultPath();
    state.currentVaultPath = null;
    showSetupScreen();
  }
}

async function boot() {
  showLoading('Starting Cypheria…');
  wireEvents();

  const hasTauri = await initTauri();

  if (hasTauri) {
    await setupTauriEvents();
  }

  hideLoading();

  if (!hasTauri) {
    showSetupScreen();
    showToast('Running without Tauri backend (preview mode)', 'warning');
    return;
  }

  await checkInitialState();
}

document.addEventListener('DOMContentLoaded', () => {
  initSmartPanel();
  boot().catch(err => {
    console.error('[Cypheria] Boot failed:', err);
    hideLoading();
    showSetupScreen();
  });
});
