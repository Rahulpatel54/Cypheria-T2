// dist/js/modules/utils.js

'use strict';

import { state } from './state.js';
import { rawInvoke } from './bridge.js';

export function showToast(msg, type = '') {
  const c = document.getElementById('toast-container');
  if (!c) return;
  const t = document.createElement('div');
  t.className = 'toast ' + type;
  const ico = type === 'success' ? '✓' : type === 'warning' ? '⚠' : type === 'error' ? '✕' : 'ℹ';
  const is = document.createElement('span');
  is.style.opacity = '0.7';
  is.textContent = ico;
  const ms = document.createElement('span');
  ms.textContent = msg;
  t.appendChild(is);
  t.appendChild(ms);
  c.appendChild(t);

  // Push clip indicator up so it clears the new toast
  const ind = document.getElementById('clip-indicator');
  if (ind && ind.style.display !== 'none') {
    ind.style.bottom = (20 + c.children.length * 52) + 'px';
  }

  setTimeout(() => {
    t.style.animation = 'toastOut 0.2s ease forwards';
    setTimeout(() => {
      t.remove();
      // Reposition clip indicator back down as toasts leave
      if (ind && ind.style.display !== 'none') {
        const remaining = c.children.length;
        ind.style.bottom = remaining > 0 ? (20 + remaining * 52) + 'px' : '20px';
      }
    }, 200);
  }, 3500);
}

export function fmtDate(iso) {
  if (!iso) return '—';
  try {
    const d = new Date(iso);
    if (isNaN(d.getTime())) {
      console.warn('[Cypheria] fmtDate: invalid timestamp value:', iso);
      return '—';
    }
    return d.toLocaleDateString(navigator.language || undefined, {
      day: '2-digit', month: 'short', year: 'numeric'
    });
  } catch (e) {
    console.warn('[Cypheria] fmtDate failed for value:', iso, e);
    return '—';
  }
}

export function makeAvatar(entry, size = 28) {
  const letter = (entry.emoji || entry.name?.charAt(0) || '?').toUpperCase().slice(0, 2);
  const _rawColor = entry.color || '#8b5cf6';
  const color = /^#[0-9a-fA-F]{6}$/.test(_rawColor) ? _rawColor : '#8b5cf6';
  const r      = size <= 28 ? 7 : 10;
  const div    = document.createElement('div');
  div.className = 'site-avatar';
  div.style.setProperty('--entry-color', color);
  div.dataset.color = color;
  div.style.width  = size + 'px';
  div.style.height = size + 'px';
  div.style.borderRadius = r + 'px';
  div.style.fontSize = Math.floor(size * 0.42) + 'px';
  div.style.overflow = 'hidden';
  div.textContent = letter;
  return div;
}

export async function copyToClipboard(value, label) {
  try {
    const { vaultCall } = await import('./bridge.js');
    await vaultCall('copy_text_to_clipboard', { text: value });
    showToast(`${label} copied`, 'success');
    const { startClipCountdown } = await import('./ui.js');
    startClipCountdown();
  } catch (e) {
    showToast('Copy failed — vault must be unlocked to copy', 'error');
  }
}

export function pwdStrength(pwd) {
  if (!pwd) return { score: 0, label: '', color: 'var(--text-muted)' };
  let s = 0;
  if (pwd.length >= 8) s += 20;
  if (pwd.length >= 12) s += 10;
  if (pwd.length >= 16) s += 10;
  if (pwd.length >= 24) s += 10;
  if (/[A-Z]/.test(pwd)) s += 15;
  if (/[a-z]/.test(pwd)) s += 15;
  if (/[0-9]/.test(pwd)) s += 10;
  if (/[^A-Za-z0-9]/.test(pwd)) s += 10;
  s = Math.min(s, 100);
  if (s < 30) return { score: s, label: 'Very Weak', color: 'var(--color-red)' };
  if (s < 50) return { score: s, label: 'Weak', color: 'var(--color-red)' };
  if (s < 65) return { score: s, label: 'Moderate', color: 'var(--color-amber)' };
  if (s < 80) return { score: s, label: 'Strong', color: 'var(--color-blue)' };
  return { score: s, label: 'Very Strong', color: 'var(--color-green)' };
}

export function toggleEye(inputId) {
  const inp = document.getElementById(inputId);
  if (inp) inp.type = inp.type === 'password' ? 'text' : 'password';
}

export function openModal(id) {
  document.getElementById(id).classList.add('open');
}

export function closeModal(id) {
  document.getElementById(id).classList.remove('open');
}