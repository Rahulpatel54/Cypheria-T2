use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Settings {
    pub theme: String,
    pub launch_at_startup: bool,
    pub minimize_to_tray: bool,
    pub auto_lock_secs: u64,
    pub show_password_default: bool,
    pub clear_clipboard_secs: u64,
    pub lock_on_blur: bool,
    pub expiry_days: u64,
    /// When true (default), the window uses OS-level content protection to block
    /// screenshots and screen recording. When false, protection is disabled.
    #[serde(default = "Settings::default_screenshot_protection")]
    pub screenshot_protection: bool,
}

impl Settings {
    fn default_screenshot_protection() -> bool {
        true
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            launch_at_startup: true,
            minimize_to_tray: true,
            auto_lock_secs: 300,
            show_password_default: false,
            clear_clipboard_secs: 30,
            lock_on_blur: false,
            expiry_days: 90,
            screenshot_protection: true,
        }
    }
}
