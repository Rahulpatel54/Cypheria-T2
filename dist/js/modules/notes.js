'use strict';

import { state } from './state.js';
import { vaultCall } from './bridge.js';
import { showToast, fmtDate, openModal, closeModal } from './utils.js';
import { confirmDelete } from './vault.js';

export async function loadNotes() {
  try {
    state.vaultNotes = await vaultCall('get_all_notes') || [];
    renderNotes();
  } catch (_) { }
}

export function renderNotes() {
  const grid = document.getElementById('notes-grid');
  const empty = document.getElementById('notes-empty');
  if (!grid) return;
  grid.innerHTML = '';
  if (!state.vaultNotes.length) { if (empty) empty.style.display = ''; return; }
  if (empty) empty.style.display = 'none';
  state.vaultNotes.forEach(n => {
    const card = document.createElement('div'); card.className = 'note-card';
    const t = document.createElement('div'); t.className = 'note-title'; t.textContent = n.title;
    const p = document.createElement('div');
    p.className = 'note-preview';
    p.textContent = '• • • • • • • • • • • • • • •';
    p.style.color = 'var(--text-muted)';
    p.style.letterSpacing = '0.15em';
    const d = document.createElement('div'); d.className = 'note-date'; d.textContent = fmtDate(n.updated_at);
    const del = document.createElement('div'); del.className = 'icon-btn note-delete'; del.title = 'Delete note';
    del.innerHTML = '<svg viewBox="0 0 24 24"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/></svg>';
    del.onclick = ev => { ev.stopPropagation(); confirmDelete('note', n.id, n.title); };
    card.appendChild(t); card.appendChild(p); card.appendChild(d); card.appendChild(del);
    card.onclick = () => openNoteModal(n);
    grid.appendChild(card);
  });
}

export async function openNoteModal(note = null) {
  document.getElementById('note-id').value = note?.id || '';
  document.getElementById('note-title').value = note?.title || '';
  document.getElementById('note-content').value = '';
  document.getElementById('note-modal-title').textContent = note ? 'Edit Note' : 'New Note';
  document.getElementById('note-error').textContent = '';

  if (note?.id) {
    try {
      const full = await vaultCall('get_note_content', { noteId: note.id });
      document.getElementById('note-content').value = full.content || '';
    } catch (e) {
      document.getElementById('note-error').textContent =
        'Could not load note content: ' + String(e).replace(/^Error: /, '').slice(0, 120);
    }
  }
  openModal('modal-note');
}

export async function saveNote() {
  const id = document.getElementById('note-id').value || null;
  const title = document.getElementById('note-title').value.trim();
  const content = document.getElementById('note-content').value;
  const errEl = document.getElementById('note-error');
  if (errEl) errEl.textContent = '';

  if (content.length > 1048576) {
    if (errEl) errEl.textContent = 'Note content is too long (max 1 MB). Please shorten it.';
    return;
  }
  if (!title) { if (errEl) errEl.textContent = 'Title is required'; return; }
  const btn = document.getElementById('btn-note-save'); if (btn) btn.disabled = true;
  try {
    await vaultCall('save_note', { noteId: id, input: { title, content } });
    closeModal('modal-note');
    await loadNotes();
    showToast(id ? 'Note updated' : 'Note saved', 'success');
  } catch (e) { if (errEl) errEl.textContent = String(e).replace(/^Error: /, '').slice(0, 120); }
  finally { if (btn) btn.disabled = false; }
}
