'use strict';
let _uiMod = null;
const getUI = () => _uiMod ? Promise.resolve(_uiMod) : import('./ui.js').then(m => { _uiMod = m; return m; });

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
    if (sort === 'name')     return (a.name || '').localeCompare(b.name || '');
    if (sort === 'username') return (a.username || '').localeCompare(b.username || '');
    if (sort === 'date')     return (b.updated_at || '').localeCompare(a.updated_at || '');
    return 0;
  });

  const tbody = document.getElementById('vault-tbody');
  const empty = document.getElementById('vault-empty');
  if (!tbody) return;

  if (!sorted.length) {
    tbody.innerHTML = '';
    if (empty) empty.style.display = '';
    return;
  }
  if (empty) empty.style.display = 'none';

  // Build a map of existing rows by entry id for O(1) lookup
  const existingRows = new Map();
  tbody.querySelectorAll('tr[id^="row-"]').forEach(tr => existingRows.set(tr.id.replace('row-', ''), tr));

  // Determine which ids still exist so we can remove stale rows
  const newIds = new Set(sorted.map(e => e.id));
  existingRows.forEach((tr, id) => { if (!newIds.has(id)) tr.remove(); });

  // Insert/update rows in correct sort order
  sorted.forEach((e, idx) => {
    let tr = existingRows.get(e.id);
    const isNew = !tr;

    if (isNew) {
      tr = document.createElement('tr');
      tr.id = 'row-' + e.id;
    }

    // Always sync selected/bulk state
    tr.classList.toggle('selected', e.id === state.selectedEntryId);
    tr.classList.toggle('bulk-selected', state.selectedEntryIds.has(e.id));

    // Only rebuild cell content if the row is new OR key data changed
    const dataKey = `${e.name}|${e.username}|${e.updated_at}|${e.is_favorite}|${e.color}|${e.emoji}`;
    if (isNew || tr.dataset.key !== dataKey) {
      tr.dataset.key = dataKey;
      tr.innerHTML = '';

      const tdT = document.createElement('td');
      const div = document.createElement('div'); div.className = 'td-title';
      const nm = document.createElement('span'); nm.className = 'td-name'; nm.textContent = e.name;
      div.appendChild(makeAvatar(e, 24)); div.appendChild(nm);
      if (e.username) {
        const qc = document.createElement('button');
        qc.className = 'quick-copy-user';
        qc.textContent = 'copy user';
        qc.title = `Copy username: ${e.username}`;
        qc.onclick = ev => {
          ev.stopPropagation();
          copyToClipboard(e.username, 'Username');
        };
        div.appendChild(qc);
      }
      tdT.appendChild(div);

      const tdU = document.createElement('td'); tdU.className = 'td-username'; tdU.textContent = e.username || '—';
      const tdD = document.createElement('td'); tdD.className = 'td-date'; tdD.textContent = fmtDate(e.updated_at);

      const tdS = document.createElement('td'); tdS.className = 'td-star';
      const sb = document.createElement('button');
      sb.className = 'star-btn' + (e.is_favorite ? ' starred' : '');
      sb.id = 'star-' + e.id;
      sb.innerHTML = '<svg viewBox="0 0 24 24"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>';
      sb.onclick = ev => { ev.stopPropagation(); toggleFavorite(e.id); };
      tdS.appendChild(sb);

      tr.appendChild(tdT); tr.appendChild(tdU); tr.appendChild(tdD); tr.appendChild(tdS);

      // Attach click handler for single/shift/ctrl select
      tr.onclick = (evt) => {
        if (evt.shiftKey && state.lastClickedEntryId) {
          const allRows = [...document.querySelectorAll('#vault-tbody tr[id^="row-"]')].map(r => r.id.replace('row-', ''));
          const a = allRows.indexOf(state.lastClickedEntryId);
          const b = allRows.indexOf(e.id);
          const [lo, hi] = a < b ? [a, b] : [b, a];
          allRows.slice(lo, hi + 1).forEach(id => state.selectedEntryIds.add(id));
          updateBulkToolbar();
        } else if (evt.ctrlKey || evt.metaKey) {
          state.selectedEntryIds.has(e.id) ? state.selectedEntryIds.delete(e.id) : state.selectedEntryIds.add(e.id);
          state.lastClickedEntryId = e.id;
          updateBulkToolbar();
        } else {
          clearBulkSelection();
          state.lastClickedEntryId = e.id;
          selectEntry(e.id);
        }
      };
    }

    // Maintain sort order: move row to correct position if needed
    const currentAtIdx = tbody.children[idx];
    if (currentAtIdx !== tr) tbody.insertBefore(tr, currentAtIdx || null);
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
    row.onclick = () => { getUI().then(m => { m.navigate('vault'); setTimeout(() => selectEntry(e.id), 60); }).catch(() => {}); };
    container.appendChild(row);
  });
  const expiredCount = Object.values(state.passwordScores).filter(s => s?.expired).length;
  const sub = document.querySelector('#page-dashboard .page-subtitle');
  if (sub) {
    sub.textContent = expiredCount > 0
      ? `${expiredCount} password${expiredCount > 1 ? 's' : ''} may need updating`
      : 'Welcome back — your vault is secure';
    sub.style.color = expiredCount > 0 ? 'var(--color-amber)' : '';
  }
  renderStaleAlerts();
}

export function renderStaleAlerts() {
  const alertsWrap = document.getElementById('dash-stale-alerts');
  const list       = document.getElementById('dash-stale-list');
  const countEl    = document.getElementById('dash-stale-count');
  if (!alertsWrap || !list || !countEl) return;

  const expiryDays = state.expiryDays || 0;
  if (expiryDays === 0) {
    // Expiry reminders disabled in settings — hide section entirely
    alertsWrap.style.display = 'none';
    return;
  }

  // Build stale list from metadata only — days_since_update from backend score view,
  // no password bytes are present in state.passwordScores entries.
  const staleEntries = state.vaultEntries
    .filter(e => {
      const s = state.passwordScores[e.id];
      // Only flag entries that have a password and exceed the threshold
      return s && s.has_password !== false && s.daysSince > expiryDays;
    })
    .sort((a, b) => {
      const da = state.passwordScores[a.id]?.daysSince || 0;
      const db = state.passwordScores[b.id]?.daysSince || 0;
      return db - da; // Most overdue first
    });

  if (!staleEntries.length) {
    alertsWrap.style.display = 'none';
    return;
  }

  alertsWrap.style.display = '';
  countEl.textContent = staleEntries.length + ' entr' + (staleEntries.length === 1 ? 'y' : 'ies');
  list.innerHTML = '';

  // Show up to 5 entries; user can navigate to vault for full list
  staleEntries.slice(0, 5).forEach(e => {
    const days = state.passwordScores[e.id]?.daysSince || 0;
    const months = Math.floor(days / 30);
    const ageText = months >= 1
      ? months + ' month' + (months !== 1 ? 's' : '') + ' ago'
      : days + ' day' + (days !== 1 ? 's' : '') + ' ago';

    const row = document.createElement('div');
    row.className = 'stale-alert-row';

    // Avatar uses makeAvatar which only uses entry.name/color/emoji — no secrets
    row.appendChild(makeAvatar(e, 28));

    const info = document.createElement('div');
    info.className = 'stale-alert-info';

    const name = document.createElement('div');
    name.className = 'stale-alert-name';
    name.textContent = e.name || '';

    const sub = document.createElement('div');
    sub.className = 'stale-alert-sub';
    sub.textContent = 'Last changed ' + ageText + (e.username ? ' · ' + e.username : '');

    info.appendChild(name);
    info.appendChild(sub);

    const badge = document.createElement('span');
    badge.className = 'stale-alert-badge';
    badge.textContent = 'Change password';

    row.appendChild(info);
    row.appendChild(badge);

    // Clicking navigates to vault and opens edit modal for that entry
    row.addEventListener('click', () => {
      getUI().then(m => {
        m.navigate('vault');
        setTimeout(() => openEditModal(e), 80);
      }).catch(() => {});
    });

    list.appendChild(row);
  });

  // If more than 5, show a "view all" hint
  if (staleEntries.length > 5) {
    const more = document.createElement('div');
    more.style.cssText = 'font-size:11px;color:var(--text-muted);text-align:center;padding:6px 0;';
    more.textContent = '+ ' + (staleEntries.length - 5) + ' more — go to Vault to see all';
    list.appendChild(more);
  }
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
    tr.onclick = () => { getUI().then(m => { m.navigate('vault'); setTimeout(() => selectEntry(e.id), 60); }).catch(() => {}); };
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
  if (!state.vaultEntries.length || state.auditInProgress) { return; }
  state.auditInProgress = true;
  try {
    const scoreList = await vaultCall('get_password_scores');
    const scores = {};
    scoreList.forEach(s => {
      const expiryDays  = state.expiryDays || 0;
      const isExpired   = expiryDays > 0 && s.days_since_update > expiryDays;
      scores[s.id] = {
        score:       s.score,
        pwdHash:     s.dup_tag,
        expired:     isExpired,
        daysSince:   s.days_since_update,
        hasPassword: s.has_password,
      };
    });
    state.passwordScores = scores;
    renderSecurityPanel();
    renderStaleAlerts();
  } catch (e) {
    if (!String(e).includes('locked')) console.warn('[Cypheria] loadPasswordScores failed:', e);
  } finally {
    state.auditInProgress = false;
  }
}

// Build a security panel row as DOM (no innerHTML, CSP safe)
function makeSecRow_dom(e, badgeText, badgeType, subText) {
  const row = document.createElement('div');
  row.className = 'sec-item-row';
  row.dataset.entryId = e.id;

  row.appendChild(makeAvatar(e, 28));

  const info = document.createElement('div');
  info.className = 'sec-item-info';

  const name = document.createElement('div');
  name.className = 'sec-item-name';
  name.textContent = e.name || '';

  const sub = document.createElement('div');
  sub.className = 'sec-item-sub';
  sub.textContent = subText || '';

  info.appendChild(name);
  info.appendChild(sub);

  const badge = document.createElement('span');
  badge.className = `sec-badge sec-badge-${badgeType}`;
  badge.textContent = badgeText;

  row.appendChild(info);
  row.appendChild(badge);

  return row;
}

export function renderSecurityPanel() {
  const container = document.getElementById('security-panel-body');
  if (!container) return;
  container.innerHTML = ''; // Always clear before rebuilding
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
    const expiryDays  = state.expiryDays || 180;
    const effectiveStale = expiryDays > 0 ? expiryDays : STALE_DAYS;
    if (s.expired || secDaysSince(e.updated_at) > effectiveStale) {
      staleList.push({ ...e, _days: secDaysSince(e.updated_at) });
    }
    // Use stored hash instead of plaintext password for duplicate detection
    if (s.pwdHash) {
      if (!pwdMap[s.pwdHash]) pwdMap[s.pwdHash] = [];
      pwdMap[s.pwdHash].push(e);
    }
  });

  // Build duplicate groups from hash map — no plaintext passwords involved
  const dupGroups = Object.values(pwdMap).filter(g => g.length > 1);
  const dupFlat   = dupGroups.flat();

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
    const d = document.createElement('div');
    d.className = 'sec-avatar';
    d.style.setProperty('--entry-color', color);
    d.dataset.color = color;
    d.textContent = letter;
    return d;
  }

  // Score row
  const scoreRow = document.createElement('div');
  scoreRow.className = 'sec-score-row';

  const ringDiv = document.createElement('div');
  ringDiv.className = 'sec-ring';
  const ringSvg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  ringSvg.setAttribute('width', '72'); ringSvg.setAttribute('height', '72');
  ringSvg.setAttribute('viewBox', '0 0 72 72');
  ringSvg.setAttribute('aria-hidden', 'true');
  ringSvg.style.transform = 'rotate(-90deg)';
  const ringBg = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
  ringBg.setAttribute('cx','36'); ringBg.setAttribute('cy','36'); ringBg.setAttribute('r','28');
  ringBg.setAttribute('fill','none'); ringBg.setAttribute('stroke','var(--border-mid)'); ringBg.setAttribute('stroke-width','7');
  const ringFg = document.createElementNS('http://www.w3.org/2000/svg', 'circle');
  ringFg.setAttribute('cx','36'); ringFg.setAttribute('cy','36'); ringFg.setAttribute('r','28');
  ringFg.setAttribute('fill','none'); ringFg.setAttribute('stroke', ringColor); ringFg.setAttribute('stroke-width','7');
  ringFg.setAttribute('stroke-linecap','round');
  ringFg.setAttribute('stroke-dasharray', String(ringCirc));
  ringFg.setAttribute('stroke-dashoffset', String(ringOffset));
  ringFg.style.transition = 'stroke-dashoffset 0.6s ease,stroke 0.4s';
  ringSvg.appendChild(ringBg); ringSvg.appendChild(ringFg);
  const ringCenter = document.createElement('div');
  ringCenter.className = 'sec-ring-center';
  const ringNum = document.createElement('span');
  ringNum.className = 'sec-ring-num'; ringNum.style.color = ringColor; ringNum.textContent = String(health);
  const ringLbl = document.createElement('span');
  ringLbl.className = 'sec-ring-lbl'; ringLbl.textContent = 'score';
  ringCenter.appendChild(ringNum); ringCenter.appendChild(ringLbl);
  ringDiv.appendChild(ringSvg); ringDiv.appendChild(ringCenter);

  const scoreInfo = document.createElement('div');
  scoreInfo.className = 'sec-score-info';
  const scoreLbl = document.createElement('div');
  scoreLbl.className = 'sec-score-label'; scoreLbl.textContent = label;
  const scoreDesc = document.createElement('div');
  scoreDesc.className = 'sec-score-desc'; scoreDesc.textContent = descText;
  scoreInfo.appendChild(scoreLbl); scoreInfo.appendChild(scoreDesc);
  scoreRow.appendChild(ringDiv); scoreRow.appendChild(scoreInfo);
  container.appendChild(scoreRow);

  // Bar grid
  const barGrid = document.createElement('div');
  barGrid.className = 'sec-bar-grid';
  [
    { label: 'Strong', count: counts.strong, color: 'var(--color-green)' },
    { label: 'Moderate', count: counts.moderate, color: 'var(--color-amber)' },
    { label: 'Weak', count: counts.weak, color: 'var(--color-red)' },
    { label: 'No password', count: counts.empty, color: 'var(--text-muted)' },
  ].forEach(({ label: bl, count, color: bc }) => {
    const item = document.createElement('div'); item.className = 'sec-bar-item';
    const lbl2 = document.createElement('div'); lbl2.className = 'sec-bar-label';
    lbl2.textContent = bl;
    const cnt2 = document.createElement('span'); cnt2.textContent = String(count);
    lbl2.appendChild(cnt2);
    const track = document.createElement('div'); track.className = 'sec-bar-track';
    const fill = document.createElement('div'); fill.className = 'sec-bar-fill';
    fill.style.width = pct(count) + '%'; fill.style.background = bc;
    track.appendChild(fill); item.appendChild(lbl2); item.appendChild(track);
    barGrid.appendChild(item);
  });
  container.appendChild(barGrid);


  // Needs-attention section
  const attnSection = document.createElement('div'); attnSection.className = 'sec-section';
  const attnTitle = document.createElement('div'); attnTitle.className = 'sec-section-title';
  const attnList = document.createElement('div'); attnList.className = 'sec-item-list';
  if (weakList.length > 0) {
    attnTitle.textContent = `Needs attention (${Math.min(weakList.length,5)})`;
    weakList.slice(0,5).forEach(e => {
      const isEmpty = e._tier === 'empty';
      attnList.appendChild(makeSecRow_dom(e,
        isEmpty ? 'Missing' : 'Weak',
        isEmpty ? 'gray' : 'red',
        isEmpty ? 'No password set' : 'Strength: ' + e._score + '/100'
      ));
    });
  } else {
    attnTitle.textContent = '';
    const ok = document.createElement('div'); ok.className = 'sec-empty'; ok.textContent = 'No weak passwords — nice work!';
    attnList.appendChild(ok);
  }
  attnSection.appendChild(attnTitle); attnSection.appendChild(attnList);
  container.appendChild(attnSection);

  // Duplicates section
  if (dupGroups.length > 0) {
    const div = document.createElement('div'); div.className = 'sec-divider'; container.appendChild(div);
    const dupSec = document.createElement('div'); dupSec.className = 'sec-section';
    const dupTitle = document.createElement('div'); dupTitle.className = 'sec-section-title'; dupTitle.textContent = 'Reused passwords';
    const dupList = document.createElement('div'); dupList.className = 'sec-item-list';
    dupGroups.slice(0,4).forEach(g => g.forEach(e => {
      dupList.appendChild(makeSecRow_dom(e, 'Reused', 'amber',
        'Shared with ' + (g.length-1) + ' other entr' + (g.length>2?'ies':'y')));
    }));
    dupSec.appendChild(dupTitle); dupSec.appendChild(dupList);
    container.appendChild(dupSec);
  }

  // Stale section
  if (staleList.length > 0) {
    const div2 = document.createElement('div'); div2.className = 'sec-divider'; container.appendChild(div2);
    const staleSec = document.createElement('div'); staleSec.className = 'sec-section';
    const staleTitle = document.createElement('div'); staleTitle.className = 'sec-section-title'; staleTitle.textContent = 'Not updated recently';
    const staleList2 = document.createElement('div'); staleList2.className = 'sec-item-list';
    staleList.slice(0,4).forEach(e => {
      const mo = Math.floor(e._days/30);
      staleList2.appendChild(makeSecRow_dom(e, mo+'mo old', 'amber', 'Not updated in '+mo+' months'));
    });
    staleSec.appendChild(staleTitle); staleSec.appendChild(staleList2);
    container.appendChild(staleSec);
  }

  // Tip
  const divTip = document.createElement('div'); divTip.className = 'sec-divider'; container.appendChild(divTip);
  const tipDiv = document.createElement('div'); tipDiv.className = 'sec-tip';
  const tipSvg = document.createElementNS('http://www.w3.org/2000/svg','svg');
  tipSvg.setAttribute('viewBox','0 0 24 24'); tipSvg.setAttribute('width','15'); tipSvg.setAttribute('height','15');
  tipSvg.style.cssText = 'flex-shrink:0;margin-top:1px';
  const tipPath = document.createElementNS('http://www.w3.org/2000/svg','path');
  tipPath.setAttribute('d','M12 2a7 7 0 0 1 7 7c0 2.5-1.4 4.8-3.5 6.1V17a1 1 0 0 1-1 1h-5a1 1 0 0 1-1-1v-1.9C6.4 13.8 5 11.5 5 9a7 7 0 0 1 7-7z');
  tipPath.setAttribute('fill','none'); tipPath.setAttribute('stroke','currentColor'); tipPath.setAttribute('stroke-width','2'); tipPath.setAttribute('stroke-linecap','round');
  const tipLine = document.createElementNS('http://www.w3.org/2000/svg','line');
  tipLine.setAttribute('x1','9'); tipLine.setAttribute('y1','21'); tipLine.setAttribute('x2','15'); tipLine.setAttribute('y2','21');
  tipLine.setAttribute('stroke','currentColor'); tipLine.setAttribute('stroke-width','2'); tipLine.setAttribute('stroke-linecap','round');
  tipSvg.appendChild(tipPath); tipSvg.appendChild(tipLine);
  const tipSpan = document.createElement('span'); tipSpan.textContent = tip;
  tipDiv.appendChild(tipSvg); tipDiv.appendChild(tipSpan);
  container.appendChild(tipDiv);

  // Attach click handlers to jump to edit
  // dynamic import of ui.js for navigate — vault.js cannot statically import ui.js
  container.querySelectorAll('.sec-item-row').forEach(row => {
  row.addEventListener('click', () => {
    const id = row.dataset.entryId;
    if (!id) return;

    const entry = state.vaultEntries.find(e => e.id === id);
    if (!entry) return;

    getUI().then(m => {
      m.navigate('vault');
      setTimeout(() => openEditModal(entry), 80);
    }).catch(() => {});
  });
});
}

export function renderStaleAlerts() {
  const wrap = document.getElementById('dash-stale-alerts');
  if (!wrap) return;

  const expiryDays = state.expiryDays || 0;
  if (expiryDays === 0) { wrap.style.display = 'none'; return; }

  const staleEntries = state.vaultEntries
    .filter(e => {
      const s = state.passwordScores[e.id];
      return s && s.hasPassword === true && s.daysSince > expiryDays;
    })
    .sort((a, b) => {
      const da = state.passwordScores[a.id]?.daysSince || 0;
      const db = state.passwordScores[b.id]?.daysSince || 0;
      return db - da;
    });

  if (!staleEntries.length) { wrap.style.display = 'none'; return; }

  wrap.style.display = '';
  wrap.innerHTML = '';

  // Header row
  const header = document.createElement('div');
  header.style.cssText = 'display:flex;align-items:center;justify-content:space-between;margin-bottom:10px;';
  const title = document.createElement('div');
  title.style.cssText = 'font-size:13px;font-weight:600;color:var(--color-amber);display:flex;align-items:center;gap:6px;';
  const icon = document.createElementNS('http://www.w3.org/2000/svg','svg');
  icon.setAttribute('viewBox','0 0 24 24');
  icon.setAttribute('width','14'); icon.setAttribute('height','14');
  icon.style.cssText = 'stroke:var(--color-amber);fill:none;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;flex-shrink:0;';
  const tri = document.createElementNS('http://www.w3.org/2000/svg','path');
  tri.setAttribute('d','M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z');
  const l1 = document.createElementNS('http://www.w3.org/2000/svg','line');
  l1.setAttribute('x1','12'); l1.setAttribute('y1','9'); l1.setAttribute('x2','12'); l1.setAttribute('y2','13');
  const l2 = document.createElementNS('http://www.w3.org/2000/svg','line');
  l2.setAttribute('x1','12'); l2.setAttribute('y1','17'); l2.setAttribute('x2','12.01'); l2.setAttribute('y2','17');
  icon.appendChild(tri); icon.appendChild(l1); icon.appendChild(l2);
  title.appendChild(icon);
  const titleText = document.createTextNode(
    `${staleEntries.length} password${staleEntries.length > 1 ? 's' : ''} not updated in ${expiryDays}+ days`
  );
  title.appendChild(titleText);
  header.appendChild(title);
  wrap.appendChild(header);

  // Entry rows (max 5)
  const list = document.createElement('div');
  list.style.cssText = 'display:flex;flex-direction:column;gap:5px;';
  staleEntries.slice(0, 5).forEach(e => {
    const row = document.createElement('div');
    row.style.cssText = 'display:flex;align-items:center;gap:9px;padding:7px 10px;background:var(--bg-surface);border:1px solid rgba(245,158,11,0.2);border-radius:var(--radius-sm);cursor:pointer;transition:border-color 0.15s;';
    row.onmouseenter = () => { row.style.borderColor = 'rgba(245,158,11,0.5)'; };
    row.onmouseleave = () => { row.style.borderColor = 'rgba(245,158,11,0.2)'; };
    row.appendChild(makeAvatar(e, 28));
    const info = document.createElement('div');
    info.style.cssText = 'flex:1;min-width:0;';
    const nm = document.createElement('div');
    nm.style.cssText = 'font-size:12px;font-weight:500;color:var(--text-primary);overflow:hidden;text-overflow:ellipsis;white-space:nowrap;';
    nm.textContent = e.name;
    const days = state.passwordScores[e.id]?.daysSince || 0;
    const mo = Math.floor(days / 30);
    const sub = document.createElement('div');
    sub.style.cssText = 'font-size:11px;color:var(--text-muted);margin-top:1px;';
    sub.textContent = mo > 0 ? `Not updated in ~${mo} month${mo > 1 ? 's' : ''}` : `${days} days since last update`;
    info.appendChild(nm); info.appendChild(sub);
    const badge = document.createElement('span');
    badge.style.cssText = 'font-size:10px;font-weight:600;padding:2px 7px;border-radius:4px;background:var(--color-amber-dim);color:var(--color-amber);border:1px solid rgba(245,158,11,0.3);white-space:nowrap;flex-shrink:0;';
    badge.textContent = mo + 'mo old';
    row.appendChild(info); row.appendChild(badge);
    row.onclick = () => {
      getUI().then(m => {
        m.navigate('vault');
        setTimeout(() => openEditModal(e), 80);
      }).catch(() => {});
    };
    list.appendChild(row);
  });
  wrap.appendChild(list);

  // "View all" link if more than 5
  if (staleEntries.length > 5) {
    const more = document.createElement('div');
    more.style.cssText = 'font-size:11px;color:var(--accent-light);margin-top:8px;cursor:pointer;text-align:right;';
    more.textContent = `+${staleEntries.length - 5} more — view in Vault`;
    more.onclick = () => getUI().then(m => m.navigate('vault')).catch(() => {});
    wrap.appendChild(more);
  }
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
  const ec = entry.color || '#8b5cf6';
  icon.style.setProperty('--entry-color', ec);
  icon.dataset.color = ec;
  icon.textContent = (entry.emoji || entry.name?.charAt(0) || '?').toUpperCase().slice(0, 2);
  const nameEl = document.createElement('div'); nameEl.className = 'detail-name'; nameEl.textContent = entry.name;
  const acts = document.createElement('div'); acts.className = 'detail-actions';
  const editBtn = document.createElement('div'); editBtn.className = 'icon-btn'; editBtn.title = 'Edit';
  { const _s=document.createElementNS('http://www.w3.org/2000/svg','svg'); _s.setAttribute('viewBox','0 0 24 24'); const _p1=document.createElementNS('http://www.w3.org/2000/svg','path'); _p1.setAttribute('d','M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7'); const _p2=document.createElementNS('http://www.w3.org/2000/svg','path'); _p2.setAttribute('d','M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z'); _s.appendChild(_p1); _s.appendChild(_p2); editBtn.appendChild(_s); }
  editBtn.onclick = () => openEditModal(entry);
  const delBtn = document.createElement('div'); delBtn.className = 'icon-btn'; delBtn.title = 'Delete';
  { const _s=document.createElementNS('http://www.w3.org/2000/svg','svg'); _s.setAttribute('viewBox','0 0 24 24'); const _pl=document.createElementNS('http://www.w3.org/2000/svg','polyline'); _pl.setAttribute('points','3 6 5 6 21 6'); _s.appendChild(_pl); [['M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6'],['M10 11v6'],['M14 11v6'],['M9 6V4h6v2']].forEach(([d])=>{ const _p=document.createElementNS('http://www.w3.org/2000/svg','path'); _p.setAttribute('d',d); _s.appendChild(_p); }); delBtn.appendChild(_s); }
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
          if (state.passwordRevealTimers.has(entry.id)) {
            clearTimeout(state.passwordRevealTimers.get(entry.id));
            state.passwordRevealTimers.delete(entry.id);
          }
        } else {
          eyeB.style.opacity = '0.5';
          try {
            const token = await vaultCall('request_reveal_token', { entryId: f.entryId });
            const pwd = await vaultCall('consume_reveal_token', { token });
            val.textContent = pwd;
          } catch (e) {
            eyeB.style.opacity = '';
            showToast('Failed to reveal password', 'error');
            return;
          }
          visible = true;
          eyeB.style.opacity = '';
          if (state.passwordRevealTimers.has(entry.id)) {
            clearTimeout(state.passwordRevealTimers.get(entry.id));
          }
          // 5s auto-hide reduces exposure window
          const tid = setTimeout(() => {
            val.textContent = '••••••••••••••••';
            visible = false;
            state.passwordRevealTimers.delete(entry.id);
          }, 5000);
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
  // Reset appearance picker to defaults
  setPickerColor('add', '#8b5cf6');
  setPickerEmoji('add', '?');
  document.getElementById('add-category').value = 'general';
  document.getElementById('add-emoji-picker').classList.remove('open');
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
    const pickedColor = document.querySelector('#add-color-swatches .color-swatch.active')?.dataset.color || '#8b5cf6';
    const pickedEmoji = document.getElementById('add-emoji-display').textContent.trim();
    const pickedCat   = document.getElementById('add-category').value;
    const input = {
      name: title, username, password, website, notes,
      is_favorite: false,
      category: pickedCat,
      color: pickedColor,
      emoji: pickedEmoji === '?' ? title.charAt(0).toUpperCase() : pickedEmoji,
    };
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
  // Restore appearance from entry
  setPickerColor('edit', entry.color || '#8b5cf6');
  setPickerEmoji('edit', entry.emoji || (entry.name?.charAt(0).toUpperCase() || '?'));
  document.getElementById('edit-category').value = entry.category || 'general';
  document.getElementById('edit-emoji-picker').classList.remove('open');
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
    const pickedColor = document.querySelector('#edit-color-swatches .color-swatch.active')?.dataset.color || existing?.color || '#8b5cf6';
    const pickedEmoji = document.getElementById('edit-emoji-display').textContent.trim();
    const pickedCat   = document.getElementById('edit-category').value;
    await vaultCall('update_entry_keep_password', {
      entryId: id,
      name: title,
      username,
      newPassword: newPwd || null,
      website,
      notes,
      isFavorite: existing?.is_favorite ?? false,
      category: pickedCat,
      color: pickedColor,
      emoji: pickedEmoji === '?' ? (title.length > 0 ? title.charAt(0).toUpperCase() : '?') : pickedEmoji,
    });
    closeModal('modal-edit-entry');
    showToast('Entry updated', 'success');
    await loadEntries();
    setTimeout(() => selectEntry(id), 80);
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

// setPickerColor — activates the correct swatch and updates the emoji preview background
export function setPickerColor(prefix, color) {
  document.querySelectorAll(`#${prefix}-color-swatches .color-swatch`).forEach(s => {
    s.classList.toggle('active', s.dataset.color === color);
    s.style.setProperty('--swatch-color', s.dataset.color);
  });

  const display = document.getElementById(`${prefix}-emoji-display`);

  if (display) {
    display.style.setProperty('--entry-color', color);
    display.dataset.color = color;
  }
}

// setPickerEmoji — updates the emoji preview button text
export function setPickerEmoji(prefix, emoji) {
  const display = document.getElementById(`${prefix}-emoji-display`);
  if (display) display.textContent = emoji;
}

// wirePickerEvents — attaches swatch + emoji picker interactions for add/edit modals
export function wirePickerEvents() {
  ['add', 'edit'].forEach(prefix => {
    // Color swatches
    document.getElementById(`${prefix}-color-swatches`)?.addEventListener('click', e => {
      const swatch = e.target.closest('.color-swatch');
      if (!swatch) return;
      setPickerColor(prefix, swatch.dataset.color);
    });

    // Emoji display toggles picker
    document.getElementById(`${prefix}-emoji-display`)?.addEventListener('click', () => {
      document.getElementById(`${prefix}-emoji-picker`).classList.toggle('open');
    });

    // Emoji selection
    document.getElementById(`${prefix}-emoji-picker`)?.addEventListener('click', e => {
      const opt = e.target.closest('.emoji-opt');
      if (!opt) return;
      setPickerEmoji(prefix, opt.textContent.trim());
      document.getElementById(`${prefix}-emoji-picker`).classList.remove('open');
    });
  });

  // Close emoji pickers when clicking outside
  document.addEventListener('click', e => {
    ['add', 'edit'].forEach(prefix => {
      const picker  = document.getElementById(`${prefix}-emoji-picker`);
      const display = document.getElementById(`${prefix}-emoji-display`);
      if (picker && display && !picker.contains(e.target) && e.target !== display) {
        picker.classList.remove('open');
      }
    });
  });
}

export function updateBulkToolbar() {
  const toolbar = document.getElementById('bulk-toolbar');
  const label   = document.getElementById('bulk-count-label');
  const count   = state.selectedEntryIds.size;
  if (toolbar) toolbar.classList.toggle('visible', count > 0);
  if (label)   label.textContent = `${count} entr${count === 1 ? 'y' : 'ies'} selected`;
  // Reflect bulk-selected class on rows
  document.querySelectorAll('#vault-tbody tr[id^="row-"]').forEach(tr => {
    const id = tr.id.replace('row-', '');
    tr.classList.toggle('bulk-selected', state.selectedEntryIds.has(id));
  });
}

// clearBulkSelection — deselects all entries and hides toolbar
export function clearBulkSelection() {
  state.selectedEntryIds.clear();
  state.lastClickedEntryId = null;
  updateBulkToolbar();
}

// wireBulkToolbar — attaches bulk action button handlers; call once from wireEvents
export function wireBulkToolbar() {
  document.getElementById('bulk-clear-btn')?.addEventListener('click', clearBulkSelection);

  document.getElementById('bulk-favorite-btn')?.addEventListener('click', async () => {
    const ids = [...state.selectedEntryIds];
    await Promise.allSettled(ids.map(id => vaultCall('toggle_favorite', { entryId: id })));
    await loadEntries();
    clearBulkSelection();
    showToast(`Updated ${ids.length} entr${ids.length === 1 ? 'y' : 'ies'}`, 'success');
  });

  document.getElementById('bulk-delete-btn')?.addEventListener('click', () => {
    const ids   = [...state.selectedEntryIds];
    const count = ids.length;
    document.getElementById('confirm-title').textContent = `Delete ${count} Entries?`;
    document.getElementById('confirm-msg').textContent   = `${count} entries will be permanently deleted. This cannot be undone.`;
    state.confirmCallback = async () => {
      await Promise.allSettled(ids.map(id => vaultCall('delete_entry', { entryId: id })));
      await loadEntries();
      if (ids.includes(state.selectedEntryId)) {
        state.selectedEntryId = null;
        const detail = document.getElementById('vault-detail');
        if (detail) detail.innerHTML = '<div class="empty-state" style="margin-top:40px;"><p>Select an entry<br>to view details</p></div>';
      }
      clearBulkSelection();
    };
    openModal('modal-confirm');
  });
}