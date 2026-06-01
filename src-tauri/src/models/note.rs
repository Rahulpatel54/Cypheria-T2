// BUG-CRIT-002 FIX: NoteView no longer contains plaintext content in bulk list.
// content_masked signals the frontend to call get_note_content() explicitly.
use serde::{Deserialize, Serialize};

/// Input from the frontend for add/update operations.
#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub title: String,
    pub content: String,
}

/// Decrypted note metadata sent TO the frontend for the notes grid.
/// content is intentionally excluded — fetch via get_note_content().
#[derive(Debug, Serialize)]
pub struct NoteView {
    pub id: String,
    pub created_at: String,
    pub updated_at: String,
    pub title: String,
    /// Always true — signals frontend to fetch content separately.
    pub content_masked: bool,
}

/// Full note returned only by get_note_content() on explicit user request.
#[derive(Debug, Serialize)]
pub struct NoteContentView {
    pub id: String,
    pub title: String,
    pub content: String,
}
