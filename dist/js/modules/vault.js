'use strict';

import { state } from './state.js';
import { vaultCall } from './bridge.js';
import { showToast, fmtDate, makeAvatar, copyToClipboard, openModal, closeModal } from './utils.js';
import { navigate, startClipCountdown } from './ui.js';
// (removed — navigate called via dynamic import inside click handlers only)

export async function loadEntries() {
  try {
    state.vaultEntries = await vaultCall('get_all_entries') || [];
    renderVaultTable();
    renderDashboard();
    renderFavorites();
    const subtitle = document.getElementById('vault-subtitle');
    if (subtitle) subtitle.textContent = `${state.vaultEntries.length} entr${state.vaultEntries.length === 1 ? 'y' : 'ies'}`;
  } catch (e) { if (!String(e).includes('locked')) showToast('Failed to load entries', 'error'); }
}

export function renderVaultTable() {
  const sort = document.getElementById('vault-sort')?.value || 'name';
  const sorted = [...state.vaultEntries].sort((a, b) => {
    if (sort === 'name') return (a.name || '').localeCompare(b.name || '');
    if (sort === 'username') return (a.username || '').localeCompare(b.username || '');
    if (sort === 'date') return (b.updated_at || '').localeCompare(a.updated_at || '');
    return 0;
  });
  const tbody = document.getElementById('vault-tbody');
  const empty = document.getElementById('vault-empty');
  if (!tbody) return;
  tbody.innerHTML = '';
  if (!sorted.length) { if (empty) empty.style.display = ''; return; }
  if (empty) empty.style.display = 'none';
  sorted.forEach(e => {
    const tr = document.createElement('tr');
    tr.id = 'row-' + e.id;
    if (e.id === state.selectedEntryId) tr.classList.add('selected');

    const tdT = document.createElement('td');
    const div = document.createElement('div'); div.className = 'td-title';
    const nm = document.createElement('span'); nm.className = 'td-name'; nm.textContent = e.name;
    div.appendChild(makeAvatar(e, 24)); div.appendChild(nm); tdT.appendChild(div);

    const tdU = document.createElement('td'); tdU.className = 'td-username'; tdU.textContent = e.username || '—';
    const tdD = document.createElement('td'); tdD.className = 'td-date'; tdD.textContent = fmtDate(e.updated_at);
    const tdS = document.createElement('td'); tdS.className = 'td-star';
    const sb = document.createElement('button'); sb.className = 'star-btn' + (e.is_favorite ? ' starred' : '');
    sb.id = 'star-' + e.id;
    sb.innerHTML = '<svg viewBox="0 0 24 24"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>';
    sb.onclick = ev => { ev.stopPropagation(); toggleFavorite(e.id); };
    tdS.appendChild(sb);

    tr.appendChild(tdT); tr.appendChild(tdU); tr.appendChild(tdD); tr.appendChild(tdS);
    tr.onclick = () => selectEntry(e.id);
    tbody.appendChild(tr);
  });
}

export function renderDashboard() {
  const dashTotal = document.getElementById('dash-total');
  if (dashTotal) dashTotal.textContent = state.vaultEntries.length;
  const dashFavs = document.getElementById('dash-favs');
  if (dashFavs) dashFavs.textContent = state.vaultEntries.filter(e => e.is_favorite).length;
  const dashNotes = document.getElementById('dash-notes');
  if (dashNotes) dashNotes.textContent = state.vaultNotes.length;

  const container = document.getElementById('recent-entries-list');
  if (!container) return;
  container.innerHTML = '';
  if (!state.vaultEntries.length) {
    container.innerHTML = '<div class="empty-state" style="padding:30px 20px;"><svg viewBox="0 0 24 24" style="width:36px;height:36px;stroke:var(--border-mid);fill:none;stroke-width:1.5;margin-bottom:12px;stroke-linecap:round;stroke-linejoin:round;"><rect x="3" y="11" width="18" height="11" rx="2" ry="2"/><path d="M7 11V7a5 5 0 0 1 10 0v4"/></svg><p>No entries yet.</p></div>';
    return;
  }
  [...state.vaultEntries].slice(0, 5).forEach(e => {
    const row = document.createElement('div'); row.className = 'recent-row';
    const info = document.createElement('div'); info.className = 'recent-info';
    const n = document.createElement('div'); n.className = 'recent-name'; n.textContent = e.name;
    const u = document.createElement('div'); u.className = 'recent-user'; u.textContent = e.username || '—';
    info.appendChild(n); info.appendChild(u);
    const d = document.createElement('div'); d.className = 'recent-date'; d.textContent = fmtDate(e.updated_at);
    row.appendChild(makeAvatar(e)); row.appendChild(info); row.appendChild(d);
    row.onclick = () => { navigate('vault'); setTimeout(() => selectEntry(e.id), 60); };
    container.appendChild(row);
  });
}

export function renderFavorites() {
  const sort = document.getElementById('fav-sort')?.value || 'name';
  const favs = state.vaultEntries.filter(e => e.is_favorite).sort((a, b) => {
    if (sort === 'name') return (a.name || '').localeCompare(b.name || '');
    return (b.updated_at || '').localeCompare(a.updated_at || '');
  });
  const tbody = document.getElementById('fav-tbody');
  const empty = document.getElementById('fav-empty');
  if (!tbody) return;
  tbody.innerHTML = '';
  if (!favs.length) { if (empty) empty.style.display = ''; return; }
  if (empty) empty.style.display = 'none';
  favs.forEach(e => {
    const tr = document.createElement('tr');
    const tdT = document.createElement('td');
    const div = document.createElement('div'); div.className = 'td-title';
    const nm = document.createElement('span'); nm.className = 'td-name'; nm.textContent = e.name;
    div.appendChild(makeAvatar(e, 24)); div.appendChild(nm); tdT.appendChild(div);
    const tdU = document.createElement('td'); tdU.className = 'td-username'; tdU.textContent = e.username || '—';
    const tdD = document.createElement('td'); tdD.className = 'td-date'; tdD.textContent = fmtDate(e.updated_at);
    const tdS = document.createElement('td'); tdS.className = 'td-star';
    const sb = document.createElement('button');
    sb.className = 'star-btn starred';
    sb.innerHTML = '<svg viewBox="0 0 24 24"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>';
    sb.onclick = ev => { ev.stopPropagation(); toggleFavorite(e.id); };
    tdS.appendChild(sb);
    tr.appendChild(tdT); tr.appendChild(tdU); tr.appendChild(tdD); tr.appendChild(tdS);
    tr.onclick = () => { navigate('vault'); setTimeout(() => selectEntry(e.id), 60); };
    tbody.appendChild(tr);
  });
}
// ── Security audit: fetch all passwords in batches, compute scores, render panel ──
const BATCH_SIZE = 5;
const STALE_DAYS = 180;

function secPwdScore(pwd) {
  if (!pwd) return 0;
  let s = 0;
  if (pwd.length >= 8)  s += 20;
  if (pwd.length >= 12) s += 10;
  if (pwd.length >= 16) s += 10;
  if (pwd.length >= 24) s += 10;
  if (/[A-Z]/.test(pwd)) s += 15;
  if (/[a-z]/.test(pwd)) s += 15;
  if (/[0-9]/.test(pwd)) s += 10;
  if (/[^A-Za-z0-9]/.test(pwd)) s += 10;
  return Math.min(s, 100);
}

function secTier(score) {
  if (score === 0)  return 'empty';
  if (score < 50)   return 'weak';
  if (score < 75)   return 'moderate';
  return 'strong';
}

function secDaysSince(iso) {
  if (!iso) return 9999;
  return Math.floor((Date.now() - new Date(iso).getTime()) / 86400000);
}

export async function loadPasswordScores() {
  // skip if no entries or vault locked
  if (!state.vaultEntries.length) { state.passwordScores = {}; return; }
  const scores = {};
  const entries = [...state.vaultEntries];
  for (let i = 0; i < entries.length; i += BATCH_SIZE) {
    const batch = entries.slice(i, i + BATCH_SIZE);
    await Promise.allSettled(batch.map(async e => {
      try {
        const pwd = await vaultCall('get_entry_password', { entryId: e.id });
        scores[e.id] = { pwd, score: secPwdScore(pwd) };
      } catch (_) {
        scores[e.id] = { pwd: '', score: 0 };
      }
    }));
    // small yield between batches to avoid blocking the event loop
    if (i + BATCH_SIZE < entries.length) await new Promise(r => setTimeout(r, 20));
  }
  state.passwordScores = scores;
  renderSecurityPanel();
}

export function renderSecurityPanel() {
  const container = document.getElementById('security-panel-body');
  if (!container) return;
  const entries = state.vaultEntries;
  const scores  = state.passwordScores;
  const total   = entries.length;

  if (total === 0) {
    container.innerHTML = '<div class="sec-empty">Add entries to see your security score.</div>';
    return;
  }

  // ── Tally buckets ──
  const counts = { strong: 0, moderate: 0, weak: 0, empty: 0 };
  const weakList = [], staleList = [], pwdMap = {};

  entries.forEach(e => {
    const s = scores[e.id];
    if (!s) return; // not yet fetched
    const t = secTier(s.score);
    counts[t]++;
    if (t === 'weak' || t === 'empty') weakList.push({ ...e, _score: s.score, _tier: t });
    if (secDaysSince(e.updated_at) > STALE_DAYS) staleList.push({ ...e, _days: secDaysSince(e.updated_at) });
    if (s.pwd) {
      if (!pwdMap[s.pwd]) pwdMap[s.pwd] = [];
      pwdMap[s.pwd].push(e);
    }
  });

  const dupGroups   = Object.values(pwdMap).filter(g => g.length > 1);
  const dupFlat     = dupGroups.flat();
  const pct         = n => total > 0 ? Math.round(n / total * 100) : 0;
  const penaltyWeak = Math.min(counts.weak * 20 + counts.empty * 30, 50);
  const penaltyDup  = Math.min(dupFlat.length * 10, 30);
  const penaltyStale= Math.min(staleList.length * 5, 15);
  const bonusStrong = Math.round(counts.strong / total * 40);
  const health = Math.max(0, Math.min(100, 50 + bonusStrong - penaltyWeak - penaltyDup - penaltyStale));

  // ── Ring colours mapped to Cypheria tokens ──
  const ringColor = health >= 75 ? 'var(--color-green)' : health >= 50 ? 'var(--color-amber)' : 'var(--color-red)';
  const ringCirc  = 175.9;
  const ringOffset= ringCirc - (health / 100) * ringCirc;
  const label     = health >= 80 ? 'Excellent' : health >= 60 ? 'Good' : health >= 40 ? 'Fair' : 'Needs work';

  const issueParts = [];
  if (counts.weak  > 0)    issueParts.push(counts.weak + ' weak');
  if (counts.empty > 0)    issueParts.push(counts.empty + ' missing');
  if (dupFlat.length > 0)  issueParts.push(dupFlat.length + ' reused');
  if (staleList.length > 0) issueParts.push(staleList.length + ' stale');
  const descText = issueParts.length ? issueParts.join(' · ') : counts.strong + ' of ' + total + ' entries are strong';

  // ── Tips pool (priority-ordered by most urgent issue) ──
  const tips = counts.empty > 0
    ? ['You have ' + counts.empty + ' entries without passwords. Add passwords to protect those accounts.']
    : counts.weak > 0
    ? ['Use the generator (16+ chars, mixed + symbols) to replace weak passwords one by one.']
    : dupFlat.length > 0
    ? ['Never reuse passwords — a breach at one site exposes every account using the same password.']
    : ['Great habits! Review stale passwords every 6 months, especially email and banking accounts.'];
  tips.push(
    'Enable 2FA on critical accounts (email, banking, cloud) for a second layer of protection.',
    'A strong password is 16+ characters with uppercase, lowercase, numbers, and symbols.',
    'Check entries with no website set — they may be orphaned accounts worth reviewing.'
  );
  const tip = tips[Math.floor(Math.random() * tips.length)];

  // ── Build markup ──
  function makeSecAvatar(e) {
    const letter = (e.emoji || e.name?.charAt(0) || '?').toUpperCase().slice(0, 2);
    const color  = e.color || '#8b5cf6';
    return `<div class="sec-avatar" style="background:${color}22;color:${color};border:1px solid ${color}44">${letter}</div>`;
  }

  function makeSecRow(e, badgeTxt, badgeCls, sub) {
    return `<div class="sec-item-row" data-entry-id="${e.id}">
      ${makeSecAvatar(e)}
      <div class="sec-item-info">
        <div class="sec-item-name">${escSecHTML(e.name || 'Untitled')}</div>
        <div class="sec-item-sub">${escSecHTML(sub)}</div>
      </div>
      <span class="sec-badge sec-badge-${badgeCls}">${badgeTxt}</span>
    </div>`;
  }

  // Action list (weak + missing, up to 5)
  const actionRows = weakList.slice(0, 5).map(e => {
    const isEmpty = e._tier === 'empty';
    return makeSecRow(e,
      isEmpty ? 'Missing' : 'Weak',
      isEmpty ? 'gray'   : 'red',
      isEmpty ? 'No password set' : 'Strength: ' + e._score + '/100'
    );
  }).join('');

  // Duplicates
  const dupRows = dupGroups.slice(0, 4).flatMap(g =>
    g.map(e => makeSecRow(e, 'Reused', 'amber',
      'Shared with ' + (g.length - 1) + ' other entr' + (g.length > 2 ? 'ies' : 'y')
    ))
  ).join('');

  // Stale
  const staleRows = staleList.slice(0, 4).map(e => {
    const mo = Math.floor(e._days / 30);
    return makeSecRow(e, mo + 'mo old', 'amber', 'Not updated in ' + mo + ' months');
  }).join('');

  container.innerHTML = `
    <div class="sec-score-row">
      <div class="sec-ring">
        <svg width="72" height="72" viewBox="0 0 72 72" aria-hidden="true" style="transform:rotate(-90deg)">
          <circle cx="36" cy="36" r="28" fill="none" stroke="var(--border-mid)" stroke-width="7"/>
          <circle cx="36" cy="36" r="28" fill="none" stroke="${ringColor}" stroke-width="7"
            stroke-linecap="round" stroke-dasharray="${ringCirc}"
            stroke-dashoffset="${ringOffset}" style="transition:stroke-dashoffset 0.6s ease,stroke 0.4s"/>
        </svg>
        <div class="sec-ring-center">
          <span class="sec-ring-num" style="color:${ringColor}">${health}</span>
          <span class="sec-ring-lbl">score</span>
        </div>
      </div>
      <div class="sec-score-info">
        <div class="sec-score-label">${label}</div>
        <div class="sec-score-desc">${descText}</div>
      </div>
    </div>

    <div class="sec-bar-grid">
      <div class="sec-bar-item">
        <div class="sec-bar-label">Strong <span>${counts.strong}</span></div>
        <div class="sec-bar-track"><div class="sec-bar-fill" style="width:${pct(counts.strong)}%;background:var(--color-green)"></div></div>
      </div>
      <div class="sec-bar-item">
        <div class="sec-bar-label">Moderate <span>${counts.moderate}</span></div>
        <div class="sec-bar-track"><div class="sec-bar-fill" style="width:${pct(counts.moderate)}%;background:var(--color-amber)"></div></div>
      </div>
      <div class="sec-bar-item">
        <div class="sec-bar-label">Weak <span>${counts.weak}</span></div>
        <div class="sec-bar-track"><div class="sec-bar-fill" style="width:${pct(counts.weak)}%;background:var(--color-red)"></div></div>
      </div>
      <div class="sec-bar-item">
        <div class="sec-bar-label">No password <span>${counts.empty}</span></div>
        <div class="sec-bar-track"><div class="sec-bar-fill" style="width:${pct(counts.empty)}%;background:var(--text-muted)"></div></div>
      </div>
    </div>

    ${weakList.length > 0 ? `
    <div class="sec-section">
      <div class="sec-section-title">Needs attention (${Math.min(weakList.length, 5)})</div>
      <div class="sec-item-list">${actionRows}</div>
    </div>` : `
    <div class="sec-section">
      <div class="sec-item-list"><div class="sec-empty">No weak passwords — nice work!</div></div>
    </div>`}

    ${dupGroups.length > 0 ? `
    <div class="sec-divider"></div>
    <div class="sec-section">
      <div class="sec-section-title">Reused passwords</div>
      <div class="sec-item-list">${dupRows}</div>
    </div>` : ''}

    ${staleList.length > 0 ? `
    <div class="sec-divider"></div>
    <div class="sec-section">
      <div class="sec-section-title">Not updated recently</div>
      <div class="sec-item-list">${staleRows}</div>
    </div>` : ''}

    <div class="sec-divider"></div>
    <div class="sec-tip">
      <svg viewBox="0 0 24 24" width="15" height="15" style="flex-shrink:0;margin-top:1px"><path d="M12 2a7 7 0 0 1 7 7c0 2.5-1.4 4.8-3.5 6.1V17a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1v-1.9C6.4 13.8 5 11.5 5 9a7 7 0 0 1 7-7z" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/><line x1="9" y1="21" x2="15" y2="21" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
      <span>${escSecHTML(tip)}</span>
    </div>
  `;

  // Attach click handlers to jump to edit
  // dynamic import of ui.js for navigate — vault.js cannot statically import ui.js
  container.querySelectorAll('.sec-item-row[data-entry-id]').forEach(row => {
    row.addEventListener('click', () => {
      const id = row.dataset.entryId;
      const entry = state.vaultEntries.find(e => e.id === id);
      if (!entry) return;
      import('./ui.js').then(m => {
        m.navigate('vault');
        setTimeout(() => openEditModal(entry), 80);
      }).catch(() => {});
    });
  });
}

function escSecHTML(s) {
  return String(s)
    .replace(/&/g,'&amp;').replace(/</g,'&lt;').replace(/>/g,'&gt;')
    .replace(/"/g,'&quot;').replace(/'/g,'&#39;');
}

export async function selectEntry(id) {
  state.passwordRevealTimers.forEach((tid) => clearTimeout(tid));
  state.passwordRevealTimers.clear();
  state.selectedEntryId = id;
  document.querySelectorAll('#vault-tbody tr').forEach(tr => tr.classList.remove('selected'));
  const row = document.getElementById('row-' + id);
  if (row) row.classList.add('selected');
  const entry = state.vaultEntries.find(e => e.id === id);
  if (!entry) return;

  const detail = document.getElementById('vault-detail');
  if (!detail) return;
  detail.innerHTML = '';

  const head = document.createElement('div'); head.className = 'detail-head';
  const icon = document.createElement('div'); icon.className = 'detail-icon';
  icon.style.cssText = `background:${entry.color || '#8b5cf6'}22;border-color:${entry.color || '#8b5cf6'}44;color:${entry.color || '#8b5cf6'};`;
  icon.textContent = (entry.emoji || entry.name?.charAt(0) || '?').toUpperCase().slice(0, 2);
  const nameEl = document.createElement('div'); nameEl.className = 'detail-name'; nameEl.textContent = entry.name;
  const acts = document.createElement('div'); acts.className = 'detail-actions';
  const editBtn = document.createElement('div'); editBtn.className = 'icon-btn'; editBtn.title = 'Edit';
  editBtn.innerHTML = '<svg viewBox="0 0 24 24"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>';
  editBtn.onclick = () => openEditModal(entry);
  const delBtn = document.createElement('div'); delBtn.className = 'icon-btn'; delBtn.title = 'Delete';
  delBtn.innerHTML = '<svg viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6M14 11v6"/><path d="M9 6V4h6v2"/></svg>';
  delBtn.onclick = () => confirmDelete('entry', entry.id, entry.name);
  acts.appendChild(editBtn); acts.appendChild(delBtn);
  head.appendChild(icon); head.appendChild(nameEl); head.appendChild(acts);
  detail.appendChild(head);

  const fields = [
    { label: 'Username', value: entry.username || '—', copy: !!entry.username },
    { label: 'Password', isPassword: true, entryId: entry.id },
    { label: 'Website', value: entry.website || '—', link: true, copy: !!entry.website },
    { label: 'Notes', value: entry.notes || '—' },
  ];
  fields.forEach(f => {
    const block = document.createElement('div'); block.className = 'field-block';
    const lbl = document.createElement('div'); lbl.className = 'field-label'; lbl.textContent = f.label;
    block.appendChild(lbl);
    const row = document.createElement('div'); row.className = 'field-value-row';
    const val = document.createElement('div'); val.className = 'field-val' + (f.link ? ' link' : '') + (f.isPassword ? ' mono' : '');

    if (f.isPassword) {
      val.textContent = '••••••••••••••••';
      let visible = false; let cachedPwd = null;
      const fa = document.createElement('div'); fa.className = 'field-actions';
      const eyeB = document.createElement('div'); eyeB.className = 'icon-btn'; eyeB.title = 'Show/hide';
      eyeB.innerHTML = '<svg viewBox="0 0 24 24"><path d="M1 12s4-8 11-8 11 8 11 8-4 8-11 8-11-8-11-8z"/><circle cx="12" cy="12" r="3"/></svg>';
      eyeB.onclick = async () => {
        if (visible) {
          val.textContent = '••••••••••••••••';
          visible = false;
          cachedPwd = null;
          if (state.passwordRevealTimers.has(entry.id)) {
            clearTimeout(state.passwordRevealTimers.get(entry.id));
            state.passwordRevealTimers.delete(entry.id);
          }
        } else {
          eyeB.style.opacity = '0.5';
          if (!cachedPwd) {
            try {
              cachedPwd = await vaultCall('get_entry_password', { entryId: f.entryId });
            } catch (e) {
              eyeB.style.opacity = '';
              showToast('Failed to reveal password', 'error');
              return;
            }
          }
          val.textContent = cachedPwd;
          visible = true;
          eyeB.style.opacity = '';
          if (state.passwordRevealTimers.has(entry.id)) {
            clearTimeout(state.passwordRevealTimers.get(entry.id));
          }
          const tid = setTimeout(() => {
            val.textContent = '••••••••••••••••';
            visible = false;
            cachedPwd = null;
            state.passwordRevealTimers.delete(entry.id);
          }, 10000);
          state.passwordRevealTimers.set(entry.id, tid);
        }
      };
      const cpyB = document.createElement('div'); cpyB.className = 'icon-btn'; cpyB.title = 'Copy password';
      cpyB.innerHTML = '<svg viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
      cpyB.onclick = async () => {
        try {
          await vaultCall('copy_entry_password_to_clipboard', {
            entryId: f.entryId,
          });
          showToast(`Password copied — clears in ${state.clipSecs || 30}s`, 'success');
          startClipCountdown();
        }
        catch (e) { showToast('Failed to copy password', 'error'); }
      };
      fa.appendChild(eyeB); fa.appendChild(cpyB);
      row.appendChild(val); row.appendChild(fa);
    } else {
      val.textContent = f.value;
      if (f.copy) {
        const fa = document.createElement('div'); fa.className = 'field-actions';
        const b = document.createElement('div'); b.className = 'icon-btn'; b.title = 'Copy';
        b.innerHTML = '<svg viewBox="0 0 24 24"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>';
        b.onclick = () => copyToClipboard(f.value, f.label);
        fa.appendChild(b); row.appendChild(val); row.appendChild(fa);
      } else { row.appendChild(val); }
    }
    block.appendChild(row);
    detail.appendChild(block);
  });

  const footer = document.createElement('div'); footer.className = 'detail-footer';
  const c = document.createElement('div'); c.className = 'detail-meta'; c.textContent = 'Created: ' + fmtDate(entry.created_at);
  const u = document.createElement('div'); u.className = 'detail-meta'; u.textContent = 'Updated: ' + fmtDate(entry.updated_at);
  footer.appendChild(c); footer.appendChild(u);
  detail.appendChild(footer);
}

export function openAddModal() {
  ['add-title', 'add-username', 'add-website', 'add-notes'].forEach(i => document.getElementById(i).value = '');
  document.getElementById('add-password').value = '';
  document.getElementById('add-error').textContent = '';
  document.getElementById('add-strength-bar').style.width = '0%';
  document.getElementById('add-strength-label').textContent = '';
  openModal('modal-add-entry');
}

export async function saveNewEntry() {
  const title = document.getElementById('add-title').value.trim();
  const username = document.getElementById('add-username').value.trim();
  const password = document.getElementById('add-password').value;
  const website = document.getElementById('add-website').value.trim();
  const notes = document.getElementById('add-notes').value.trim();
  document.getElementById('add-error').textContent = '';
  if (!title) { document.getElementById('add-error').textContent = 'Title is required'; return; }
  const btn = document.getElementById('btn-add-save'); btn.disabled = true;
  try {
    const input = { name: title, username, password, website, notes, is_favorite: false, category: 'general', color: '#8b5cf6', emoji: title.charAt(0).toUpperCase() };
    await vaultCall('add_entry', { input });
    closeModal('modal-add-entry');
    await loadEntries();
    showToast('Entry saved', 'success');
  } catch (e) {
    document.getElementById('add-error').textContent = String(e).replace(/^Error: /, '').slice(0, 120);
  } finally { btn.disabled = false; }
}

export function openEditModal(entry) {
  document.getElementById('edit-id').value = entry.id;
  document.getElementById('edit-title').value = entry.name;
  document.getElementById('edit-username').value = entry.username || '';
  document.getElementById('edit-password').value = '';
  document.getElementById('edit-website').value = entry.website || '';
  document.getElementById('edit-notes').value = entry.notes || '';
  document.getElementById('edit-error').textContent = '';
  openModal('modal-edit-entry');
}

export async function saveEditEntry() {
  const id = document.getElementById('edit-id').value;
  const title = document.getElementById('edit-title').value.trim();
  const username = document.getElementById('edit-username').value.trim();
  const newPwd = document.getElementById('edit-password').value;
  const website = document.getElementById('edit-website').value.trim();
  const notes = document.getElementById('edit-notes').value.trim();
  const errEl = document.getElementById('edit-error');
  if (errEl) errEl.textContent = '';

  if (!title) {
    if (errEl) errEl.textContent = 'Title is required';
    return;
  }

  const existing = state.vaultEntries.find(e => e.id === id);
  const btn = document.getElementById('btn-edit-save');
  if (btn) btn.disabled = true;

  try {
    await vaultCall('update_entry_keep_password', {
      entryId: id,
      name: title,
      username,
      newPassword: newPwd || null,
      website,
      notes,
      isFavorite: existing?.is_favorite ?? false,
      category: existing?.category || 'general',
      color: existing?.color || '#8b5cf6',
      emoji: existing?.emoji || (title.length > 0 ? title.charAt(0).toUpperCase() : '?'),
    });
    closeModal('modal-edit-entry');
    await loadEntries();
    setTimeout(() => selectEntry(id), 80);
    showToast('Entry updated', 'success');
  } catch (e) {
    if (errEl) errEl.textContent = String(e).replace(/^Error: /, '').slice(0, 120);
  } finally {
    if (btn) btn.disabled = false;
  }
}

export async function toggleFavorite(id) {
  try {
    const newState = await vaultCall('toggle_favorite', { entryId: id });
    const entry = state.vaultEntries.find(e => e.id === id);
    if (entry) entry.is_favorite = newState;
    const btn = document.getElementById('star-' + id);
    if (btn) btn.className = 'star-btn' + (newState ? ' starred' : '');
    const dashFavs = document.getElementById('dash-favs');
    if (dashFavs) dashFavs.textContent = state.vaultEntries.filter(e => e.is_favorite).length;
    if (document.getElementById('page-favorites').classList.contains('active')) {
      renderFavorites();
    }
  } catch (_) { showToast('Failed to update favorite', 'error'); }
}

export function confirmDelete(type, id, name) {
  document.getElementById('confirm-title').textContent = `Delete ${type === 'entry' ? 'Entry' : 'Note'}?`;
  document.getElementById('confirm-msg').textContent = `"${name}" will be permanently deleted. This cannot be undone.`;
  state.confirmCallback = type === 'entry'
    ? async () => {
      await vaultCall('delete_entry', { entryId: id });
      await loadEntries();
      if (state.selectedEntryId === id) {
        state.selectedEntryId = null;
        document.getElementById('vault-detail').innerHTML = '<div class="empty-state" style="margin-top:40px;"><p>Select an entry<br>to view details</p></div>';
      }
    }
    : async () => {
      const { loadNotes } = await import('./notes.js');
      await vaultCall('delete_note', { noteId: id });
      await loadNotes();
    };
  openModal('modal-confirm');
}
