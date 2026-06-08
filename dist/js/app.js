'use strict';

import { state } from './modules/state.js';
import { initTauri, rawInvoke, getPersistedVaultPath, persistVaultPath, clearPersistedVaultPath } from './modules/bridge.js';
import { showLoading, hideLoading, navigate, wireEvents, wireActivityListeners } from './modules/ui.js';
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
  if (!state._invoke) {
    showSetupScreen();
    return;
  }

  let stored = null;
  try {
    stored = await Promise.race([
      getPersistedVaultPath(),
      new Promise((_, reject) => setTimeout(() => reject(new Error('vault path timeout')), 3000))
    ]);
  } catch(e) {
    showSetupScreen();
    return;
  }

  if (!stored) {
    showSetupScreen();
    return;
  }

  state.currentVaultPath = stored;

  try {
    const meta = await Promise.race([
      rawInvoke('get_vault_meta', { vaultPath: stored }),
      new Promise((_, reject) => setTimeout(() => reject(new Error('meta timeout')), 3000))
    ]);
    const name = meta?.vault_name
      || stored.split(/[\\/]/).pop().replace(/\.qvault$/i, '');
    state.currentVaultName = name;
    document.getElementById('lock-vault-name').textContent = name;
  } catch(e) {
    const name = stored.split(/[\\/]/).pop().replace(/\.qvault$/i, '');
    state.currentVaultName = name;
    document.getElementById('lock-vault-name').textContent = name;
  }

  showLockScreen();
}

async function boot() {
  showLoading('Starting Cypheria…');

  try {
    wireEvents();
    wireActivityListeners();

    const hasTauri = await initTauri();

    if (hasTauri) {
      setupTauriEvents().catch(e => console.warn('[Cypheria] setupTauriEvents failed:', e));
    }

    hideLoading();

    if (!hasTauri) {
      showSetupScreen();
      showToast('Running without Tauri backend (preview mode)', 'warning');
      return;
    }

    await new Promise(resolve => setTimeout(resolve, 50));
    await checkInitialState();

  } catch (err) {
    console.error('[Cypheria] Boot error:', err);
    hideLoading();
    showSetupScreen();
  }
}

document.addEventListener('DOMContentLoaded', () => {
  // Immediately hide loader after a max of 10s no matter what
  const bootTimeout = setTimeout(() => {
    console.error('[Cypheria] Boot timed out — forcing setup screen');
    document.getElementById('loading-overlay').classList.add('hidden');
    showSetupScreen();
  }, 10000);

  // Force-hide loading after 500ms minimum so spinner is visible briefly
  // but never blocks indefinitely if JS errors occur before hideLoading()
  const forceHideTimer = setTimeout(() => {
    const overlay = document.getElementById('loading-overlay');
    if (overlay && !overlay.classList.contains('hidden')) {
      console.warn('[Cypheria] forceHide triggered — boot may have hung');
      overlay.classList.add('hidden');
      showSetupScreen();
    }
  }, 5000);

  initSmartPanel();
  try {
    if (window.__TAURI_INTERNALS__?.invoke) {
      window.__TAURI_INTERNALS__.invoke('plugin:app|version').then(v => {
        const el = document.getElementById('about-version');
        if (el && v) el.textContent = v;
      }).catch(() => {});
    }
  } catch (_) {}

  boot().catch(err => {
    console.error('[Cypheria] Boot failed:', err);
    document.getElementById('loading-overlay').classList.add('hidden');
    showSetupScreen();
  }).finally(() => {
    clearTimeout(bootTimeout);
    clearTimeout(forceHideTimer);
  });
});
