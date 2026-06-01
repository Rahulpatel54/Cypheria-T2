//! Entry commands — CRUD operations exposed to the frontend via Tauri IPC.

use crate::commands::validate_uuid;
use crate::{
    error::CypheriaError,
    models::entry::{EntryInput, EntryView},
    session::{autolock::AutoLockTimer, manager::SessionManager},
    vault::entry,
};
use std::sync::Arc;
use tauri::State;
use zeroize::Zeroize;

#[tauri::command]
pub async fn get_all_entries(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<EntryView>, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    vault_store
                        .data
                        .entries
                        .iter()
                        .map(|e| entry::decrypt_entry(key_store.vault_key_bytes(), e))
                        .collect::<Result<Vec<_>, _>>()
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn get_entry_password(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        validate_uuid(&entry_id)?;
        session.check_reveal_rate_limit()?;
        session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::get_entry_password(
                        key_store.vault_key_bytes(),
                        &vault_store.data,
                        &entry_id,
                    )
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn add_entry(
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<String, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::add_entry(key_store.vault_key_bytes(), &mut vault_store.data, input)
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn update_entry(
    entry_id: String,
    input: EntryInput,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        validate_uuid(&entry_id)?;
        if input.password.is_empty() {
            return Err(CypheriaError::InvalidInput(
                "Password cannot be empty. Use update_entry_keep_password to keep the existing password.".into()
            ));
        }
        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::update_entry(
                        key_store.vault_key_bytes(),
                        &mut vault_store.data,
                        &entry_id,
                        input,
                    )
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn delete_entry(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        // BUG-008 fix: unconditional validation.
        validate_uuid(&entry_id)?;
        session
            .with_session_mut(|_key_store, vault_store| {
                catch_sync_panic!({
                    let pre_len = vault_store.data.entries.len();
                    vault_store.data.entries.retain(|e| e.id != entry_id);
                    if vault_store.data.entries.len() == pre_len {
                        return Err(CypheriaError::EntryNotFound(entry_id.clone()));
                    }
                    // ERR-007 fix: stamp the vault-level updated_at on deletion.
                    vault_store.data.updated_at = chrono::Utc::now();
                    Ok(())
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn toggle_favorite(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<bool, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        // BUG-008 fix: unconditional validation.
        validate_uuid(&entry_id)?;
        session
            .with_session_mut(|_key_store, vault_store| {
                catch_sync_panic!({
                    let e = vault_store
                        .data
                        .entries
                        .iter_mut()
                        .find(|e| e.id == entry_id)
                        .ok_or_else(|| CypheriaError::EntryNotFound(entry_id.clone()))?;
                    e.is_favorite = !e.is_favorite;
                    Ok(e.is_favorite)
                })
            })
            .await
    })
}

// Issue 4 fix: one-use reveal token — password never returned directly over IPC from this path
#[tauri::command]
pub async fn request_reveal_token(
    entry_id: String,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
    reveal_store: State<'_, Arc<crate::commands::reveal::RevealStore>>,
) -> Result<String, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        validate_uuid(&entry_id)?;
        session.check_reveal_rate_limit()?;
        let password = session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    entry::get_entry_password(
                        key_store.vault_key_bytes(),
                        &vault_store.data,
                        &entry_id,
                    )
                })
            })
            .await?;
        let token = reveal_store.store(password);
        Ok(token)
    })
}

// Issue 4 fix: consume the one-use token and return the password — token is deleted immediately
#[tauri::command]
pub async fn consume_reveal_token(
    token: String,
    reveal_store: State<'_, Arc<crate::commands::reveal::RevealStore>>,
) -> Result<String, CypheriaError> {
    safe_command!({
        reveal_store
            .consume(&token)
            .ok_or(CypheriaError::InvalidInput(
                "Invalid or expired token".into(),
            ))
    })
}

#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn update_entry_keep_password(
    entry_id: String,
    name: String,
    username: String,
    new_password: Option<String>,
    website: String,
    notes: String,
    is_favorite: Option<bool>,
    category: Option<String>,
    color: Option<String>,
    emoji: Option<String>,
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<(), CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        validate_uuid(&entry_id)?;

        session
            .with_session_mut(|key_store, vault_store| {
                catch_sync_panic!({
                    let vk = key_store.vault_key_bytes();

                    let password = match new_password.as_deref() {
                        Some(p) if !p.is_empty() => p.to_string(),
                        _ => entry::get_entry_password(vk, &vault_store.data, &entry_id)?,
                    };

                    let input = crate::models::entry::EntryInput {
                        name,
                        username,
                        password,
                        website,
                        notes,
                        is_favorite,
                        category,
                        color,
                        emoji,
                    };

                    entry::update_entry(vk, &mut vault_store.data, &entry_id, input)
                })
            })
            .await
    })
}

#[tauri::command]
pub async fn get_password_scores(
    session: State<'_, Arc<SessionManager>>,
    autolock: State<'_, Arc<AutoLockTimer>>,
) -> Result<Vec<crate::models::entry::EntryScoreView>, CypheriaError> {
    safe_command!({
        autolock.bump_activity();
        session
            .with_session(|key_store, vault_store| {
                catch_sync_panic!({
                    use hmac::{Hmac, Mac};
                    use sha2::Sha256;

                    // Domain key for duplicate detection — separate from vault HMAC key
                    let mut dup_key = [0u8; 32];
                    crate::crypto::kdf::derive_subkey(
                        key_store.vault_key_bytes(),
                        b"PWD_DUP_DETECTION",
                        &mut dup_key,
                    );

                    let mut scores = Vec::with_capacity(vault_store.data.entries.len());
                    for enc_entry in &vault_store.data.entries {
                        let pwd = match entry::get_entry_password(
                            key_store.vault_key_bytes(),
                            &vault_store.data,
                            &enc_entry.id,
                        ) {
                            Ok(p) => p,
                            Err(_) => {
                                scores.push(crate::models::entry::EntryScoreView {
                                    id: enc_entry.id.clone(),
                                    score: 0,
                                    has_password: false,
                                    dup_tag: String::new(),
                                    days_since_update: 0,
                                });
                                continue;
                            }
                        };

                        let score = compute_pwd_score(&pwd);
                        let has_password = !pwd.is_empty();

                        // Truncated HMAC tag (first 8 hex chars = 32 bits) for duplicate
                        // detection — enough to find duplicates, not enough to brute-force
                        let mut mac = <Hmac<Sha256>>::new_from_slice(&dup_key)
                            .map_err(|_| CypheriaError::CryptoError)?;
                        mac.update(pwd.as_bytes());
                        let tag_bytes = mac.finalize().into_bytes();
                        let dup_tag: String = tag_bytes[..4]
                            .iter()
                            .map(|b| format!("{:02x}", b))
                            .collect();

                        let days = chrono::Utc::now()
                            .signed_duration_since(enc_entry.updated_at)
                            .num_days()
                            .max(0) as u64;

                        // pwd is dropped here — not stored in EntryScoreView
                        scores.push(crate::models::entry::EntryScoreView {
                            id: enc_entry.id.clone(),
                            score,
                            has_password,
                            dup_tag,
                            days_since_update: days,
                        });
                    }
                    dup_key.zeroize();
                    Ok(scores)
                })
            })
            .await
    })
}

/// Score a password 0–100 without external dependencies.
fn compute_pwd_score(pwd: &str) -> u8 {
    if pwd.is_empty() {
        return 0;
    }
    let mut s: u32 = 0;
    if pwd.len() >= 8 {
        s += 20;
    }
    if pwd.len() >= 12 {
        s += 10;
    }
    if pwd.len() >= 16 {
        s += 10;
    }
    if pwd.len() >= 24 {
        s += 10;
    }
    if pwd.chars().any(|c| c.is_ascii_uppercase()) {
        s += 15;
    }
    if pwd.chars().any(|c| c.is_ascii_lowercase()) {
        s += 15;
    }
    if pwd.chars().any(|c| c.is_ascii_digit()) {
        s += 10;
    }
    if pwd.chars().any(|c| !c.is_alphanumeric()) {
        s += 10;
    }
    s.min(100) as u8
}
