use serde::{Serialize, Deserialize};

/// Input from the frontend for add/update operations.
#[derive(Debug, Deserialize)]
pub struct NoteInput {
    pub title:   String,
    pub content: String,
}

/// Decrypted note sent TO the frontend.
#[derive(Debug, Serialize)]
pub struct NoteView {
    pub id:         String,
    pub created_at: String,
    pub updated_at: String,
    pub title:      String,
    pub content:    String,
}
