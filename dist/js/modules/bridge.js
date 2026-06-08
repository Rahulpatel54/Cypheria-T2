'use strict';

import { state } from './state.js';

export async function initTauri() {
  try {
    let _rawInvoke = null;
    if (window.__TAURI_INTERNALS__?.invoke) {
      _rawInvoke = window.__TAURI_INTERNALS__.invoke;
    } else if (window.__TAURI__?.core?.invoke) {
      _rawInvoke = window.__TAURI__.core.invoke;
    } else if (window.__TAURI__?.tauri?.invoke) {
      _rawInvoke = window.__TAURI__.tauri.invoke;
    }
    if (_rawInvoke) {
      state._invoke = _rawInvoke.bind(window.__TAURI_INTERNALS__ ?? window);
    }
    return !!state._invoke;
  } catch (e) {
    console.warn('[Cypheria] Tauri bridge unavailable:', e.message);
    return false;
  }
}

// Session-guarded vault calls — bumps inactivity timer and resets UI countdown
export async function vaultCall(cmd, args = {}) {
  if (!state._invoke) throw new Error('Backend unavailable');
  try {
    const result = await state._invoke(cmd, args);
    // Reset the UI countdown on every successful vault command
    const _rawTimeout = parseInt(document.getElementById('set-autolock')?.value || '0', 10);
    const timeoutSecs = Number.isFinite(_rawTimeout) && _rawTimeout >= 0 && _rawTimeout <= 86400
      ? _rawTimeout : 0;
    if (timeoutSecs > 0) {
      import('./ui.js').then(m => m.bumpAutolockCountdown(timeoutSecs)).catch(() => {});
    }
    return result;
  } catch (err) {
    const msg = String(err);
    if (msg.includes('Vault is locked') || msg.includes('Session expired')) {
      const { showLockScreen } = await import('./auth.js');
      showLockScreen();
      throw err;
    }
    throw err;
  }
}

// Direct invoke — no session guard, no activity bump
export async function rawInvoke(cmd, args = {}) {
  if (!state._invoke) throw new Error('Backend unavailable');
  return await state._invoke(cmd, args);
}

export async function persistVaultPath(path) {
  if (!state._invoke || !path) return;
  try {
    await rawInvoke('set_last_vault_path', { path });
  } catch (e) {
    console.warn('[Cypheria] Could not persist vault path:', e);
  }
}

export async function getPersistedVaultPath() {
  if (!state._invoke) return null;
  try {
    return await rawInvoke('get_last_vault_path') || null;
  } catch (_) {
    return null;
  }
}

export async function clearPersistedVaultPath() {
  if (!state._invoke) return;
  try {
    await rawInvoke('clear_last_vault_path');
  } catch (_) {}
}
