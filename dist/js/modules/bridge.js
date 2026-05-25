'use strict';

import { state } from './state.js';

export async function initTauri() {
  try {
    if (window.__TAURI_INTERNALS__?.invoke) {
      state._invoke = window.__TAURI_INTERNALS__.invoke;
    } else if (window.__TAURI__?.core?.invoke) {
      state._invoke = window.__TAURI__.core.invoke;
    } else if (window.__TAURI__?.tauri?.invoke) {
      state._invoke = window.__TAURI__.tauri.invoke;
    }
    return !!state._invoke;
  } catch (e) {
    console.warn('[Cypheria] Tauri bridge unavailable:', e.message);
    return false;
  }
}

// Session-guarded vault calls (bumps inactivity timer)
export async function vaultCall(cmd, args = {}) {
  if (!state._invoke) throw new Error('Backend unavailable');
  try {
    return await state._invoke(cmd, args);
  } catch (err) {
    const msg = String(err);
    if (msg.includes('Vault is locked') || msg.includes('Session expired')) {
      // Import showLockScreen dynamically to avoid circular dependency
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
