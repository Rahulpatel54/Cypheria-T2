'use strict';

import { state } from './state.js';
import { rawInvoke } from './bridge.js';
import { showToast, copyToClipboard, pwdStrength } from './utils.js';

export function generatePassword() {
  const length = parseInt(document.getElementById('gen-length')?.value || '20');
  const upper = document.getElementById('gen-upper')?.checked ?? true;
  const lower = document.getElementById('gen-lower')?.checked ?? true;
  const numbers = document.getElementById('gen-numbers')?.checked ?? true;
  const symbols = document.getElementById('gen-symbols')?.checked ?? true;
  if (!upper && !lower && !numbers && !symbols) { showToast('Select at least one character set', 'warning'); return; }
  if (state._invoke) {
    rawInvoke('generate_password', { options: { length, upper, lower, numbers, symbols } })
      .then(r => {
        const pwd = r?.password || r;
        const entropy = r?.entropy_bits;
        const strength = r?.strength;
        const out = document.getElementById('gen-output');
        if (out) out.textContent = pwd;
        updateGenStrength(pwd, entropy, strength);
      })
      .catch(() => {
        const pwd = clientGenPassword(length, upper, lower, numbers, symbols);
        const out = document.getElementById('gen-output');
        if (out) out.textContent = pwd;
        updateGenStrength(pwd, null, null);
      });
  } else {
    const pwd = clientGenPassword(length, upper, lower, numbers, symbols);
    const out = document.getElementById('gen-output');
    if (out) out.textContent = pwd;
    updateGenStrength(pwd, null, null);
  }
}

export function clientGenPassword(length, upper, lower, numbers, symbols) {
  let chars = '';
  if (upper) chars += 'ABCDEFGHIJKLMNOPQRSTUVWXYZ';
  if (lower) chars += 'abcdefghijklmnopqrstuvwxyz';
  if (numbers) chars += '0123456789';
  if (symbols) chars += '!@#$%^&*()_+-=[]{}|;:,.<>?';
  const max = Math.floor(0x100000000 / chars.length) * chars.length;
  let out = '';
  while (out.length < length) {
    const arr = new Uint32Array(Math.max((length - out.length) * 2, 16));
    crypto.getRandomValues(arr);
    for (let i = 0; i < arr.length && out.length < length; i++) {
      if (arr[i] < max) out += chars[arr[i] % chars.length];
    }
  }
  return out;
}

export function updateGenStrength(pwd, entropy, strength) {
  const fill = document.getElementById('gen-strength-fill');
  const lbl = document.getElementById('gen-strength-label');
  const entEl = document.getElementById('gen-entropy');
  if (!pwd || pwd === 'Click Generate') { if (fill) fill.style.width = '0%'; if (lbl) { lbl.textContent = 'Generate a password to see strength'; lbl.style.color = 'var(--text-muted)'; } return; }
  let score, label, color;
  if (entropy != null) {
    score = Math.min(100, (entropy / 128) * 100);
    label = strength || (entropy < 36 ? 'Weak' : entropy < 60 ? 'Moderate' : entropy < 80 ? 'Strong' : 'Very Strong');
    if (entEl) entEl.textContent = `${entropy} bits of entropy`;
  } else {
    const s = pwdStrength(pwd); score = s.score; label = s.label;
    if (entEl) entEl.textContent = '';
  }
  color = score < 40 ? 'var(--color-red)' : score < 60 ? 'var(--color-amber)' : score < 80 ? 'var(--color-blue)' : 'var(--color-green)';
  if (fill) { fill.style.width = score + '%'; fill.style.background = color; }
  if (lbl) { lbl.textContent = label; lbl.style.color = color; }
}

export function copGenPwd() {
  const p = document.getElementById('gen-output')?.textContent;
  if (p && p !== 'Click Generate') copyToClipboard(p, 'Password');
}
