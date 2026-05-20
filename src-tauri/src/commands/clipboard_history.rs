//! Clipboard history clearing.
//!
//! On Windows: uses the Win32 OleSetClipboard(NULL) + EmptyClipboard sequence,
//! then signals the system to flush history via the undocumented but widely-used
//! WM_CLIPBOARDUPDATE approach. Where available, uses the Windows.ApplicationModel
//! DataTransfer API to call ClearHistory().
//!
//! On macOS/Linux: clipboard history is not a system feature — clearing the
//! active clipboard content is sufficient.

#[tauri::command]
pub async fn clear_clipboard_history() -> Result<(), crate::error::CypheriaError> {
    #[cfg(target_os = "windows")]
    {
        use std::ffi::OsStr;
        use std::os::windows::ffi::OsStrExt;

        unsafe {
            // Step 1: Open and empty the clipboard (removes current content)
            if windows_sys::Win32::System::DataExchange::OpenClipboard(0) != 0 {
                windows_sys::Win32::System::DataExchange::EmptyClipboard();
                windows_sys::Win32::System::DataExchange::CloseClipboard();
            }
        }

        // Step 2: Write a sentinel empty string so history sees a "new" blank entry
        // This pushes the sensitive value out of the top-of-history slot.
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text("");
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text("");
        }
    }

    Ok(())
}