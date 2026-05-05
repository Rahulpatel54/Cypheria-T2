use serde::{Serialize, Deserialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub theme:                 String,
    pub launch_at_startup:     bool,
    pub minimize_to_tray:      bool,
    pub auto_lock_secs:        u64,
    pub show_password_default: bool,
    pub clear_clipboard_secs:  u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme:                 "dark".into(),
            launch_at_startup:     true,
            minimize_to_tray:      true,
            auto_lock_secs:        300,   // 5 minutes
            show_password_default: false,
            clear_clipboard_secs:  30,
        }
    }
}
