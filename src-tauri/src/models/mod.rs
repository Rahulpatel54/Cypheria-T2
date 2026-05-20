//! Serde-serializable data transfer objects used across the command boundary.
//!
//! These types are safe to send to / receive from the frontend.
//! They must NEVER contain raw key bytes or unencrypted vault internals.
//!
//! Sub-modules:
//!   entry     — EntryInput (from frontend), EntryView (to frontend, password excluded)
//!               GenOptions (password generator options)
//!   note      — NoteInput (from frontend), NoteView (to frontend)
//!   settings  — Settings with all user-configurable preferences
//!
//! NOTE: `models/note.rs` contains the frontend-facing DTOs only.
//!       The CRUD logic lives in `vault/notes.rs`.
//!       Do not confuse the two — they are intentionally separate.

pub mod entry;
pub mod note;
pub mod settings;
