'use strict';

import { state } from './modules/state.js';
import { initTauri, rawInvoke, getPersistedVaultPath, persistVaultPath, clearPersistedVaultPath } from './modules/bridge.js';
import { showLoading, hideLoading, navigate, wireEvents } from './modules/ui.js';
import { showToast } from './modules/utils.js';
import { showSetupScreen, showLockScreen } from './modules/auth.js';
import { initSmartPanel } from './modules/smart-panel.js';

async function setupTauriEvents() {
  try {
    const listen = window.__TAURI_INTERNALS__?.listen || window.__TAURI__?.event?.listen;
    if (listen) {
      await listen('vault-auto-locked', async () => {
          if (state._invoke) await rawInvoke('clear_clipboard').catch(() => { });
          const { clearAutolockCountdown } = await import('./modules/ui.js');
          clearAutolockCountdown();
          const { showLockScreen } = await import('./modules/auth.js');
          showLockScreen();
          showToast('Vault locked due to inactivity', 'warning');
        });
        await listen('vault-locked', async () => {
          if (state._invoke) await rawInvoke('clear_clipboard').catch(() => { });
          const { clearAutolockCountdown } = await import('./modules/ui.js');
          clearAutolockCountdown();
          const { showLockScreen } = await import('./modules/auth.js');
          showLockScreen();
        });
    }
  } catch (_) { }
}

async function checkInitialState() {
 if (!state._invoke) { showSetupScreen(); return; }

  const stored = await Promise.race([
    getPersistedVaultPath(),
    new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 5000))
  ]).catch(() => null);
  if (!stored) { showSetupScreen(); return; }

 try {
    const canonical = await Promise.race([
      rawInvoke('open_vault', { vaultPath: stored }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('timeout')), 5000))
    ]);
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
  console.log('[DEBUG] boot started');

  try {
    console.log('[DEBUG] wiring events...');
    wireEvents();
    console.log('[DEBUG] events wired');

    console.log('[DEBUG] init tauri...');
    const hasTauri = await initTauri();
    console.log('[DEBUG] hasTauri =', hasTauri);

    if (hasTauri) {
      console.log('[DEBUG] setting up tauri events...');
      await setupTauriEvents();
      console.log('[DEBUG] tauri events done');
    }

    console.log('[DEBUG] hiding loading...');
    hideLoading();

    if (!hasTauri) {
      showSetupScreen();
      showToast('Running without Tauri backend (preview mode)', 'warning');
      return;
    }

    console.log('[DEBUG] checking initial state...');
    await checkInitialState();
    console.log('[DEBUG] initial state done');

  } catch (err) {
    console.error('[DEBUG] boot error:', err);
    hideLoading();
    showSetupScreen();
  }
}

document.addEventListener('DOMContentLoaded', () => {
  // Safety net: if boot hangs for 10 seconds, force show setup screen
  const bootTimeout = setTimeout(() => {
    console.error('[Cypheria] Boot timed out');
    hideLoading();
    showSetupScreen();
  }, 10000);

  initSmartPanel();
  boot().catch(err => {
    console.error('[Cypheria] Boot failed:', err);
    hideLoading();
    showSetupScreen();
  }).finally(() => {
    clearTimeout(bootTimeout);
  });
});
