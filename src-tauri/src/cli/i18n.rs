use crate::settings::{get_settings, update_settings};
use std::sync::OnceLock;
use std::sync::RwLock;

#[cfg(test)]
use std::cell::RefCell;

/// Supported languages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    English,
    Chinese,
}

impl Language {
    pub fn code(&self) -> &'static str {
        match self {
            Language::English => "en",
            Language::Chinese => "zh",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Language::English => "English",
            Language::Chinese => "中文",
        }
    }

    pub fn from_code(code: &str) -> Self {
        match code.to_lowercase().as_str() {
            "zh" | "zh-cn" | "zh-tw" | "chinese" => Language::Chinese,
            _ => Language::English,
        }
    }
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Global language state
fn language_store() -> &'static RwLock<Language> {
    static STORE: OnceLock<RwLock<Language>> = OnceLock::new();
    STORE.get_or_init(|| {
        let lang = if cfg!(test) {
            // Keep unit tests deterministic and avoid reading real user settings.
            Language::English
        } else {
            let settings = get_settings();
            settings
                .language
                .as_deref()
                .map(Language::from_code)
                .unwrap_or(Language::English)
        };
        RwLock::new(lang)
    })
}

#[cfg(test)]
thread_local! {
    static TEST_LANGUAGE_OVERRIDE: RefCell<Option<Language>> = const { RefCell::new(None) };
}

#[cfg(test)]
struct TestLanguageGuard(Option<Language>);

#[cfg(test)]
impl Drop for TestLanguageGuard {
    fn drop(&mut self) {
        TEST_LANGUAGE_OVERRIDE.with(|slot| {
            *slot.borrow_mut() = self.0;
        });
    }
}

#[cfg(test)]
fn use_test_language(lang: Language) -> TestLanguageGuard {
    let previous = TEST_LANGUAGE_OVERRIDE.with(|slot| slot.replace(Some(lang)));
    TestLanguageGuard(previous)
}

/// Get current language
pub fn current_language() -> Language {
    #[cfg(test)]
    if let Some(lang) = TEST_LANGUAGE_OVERRIDE.with(|slot| *slot.borrow()) {
        return lang;
    }

    *language_store().read().expect("Failed to read language")
}

/// Set current language and persist
pub fn set_language(lang: Language) -> Result<(), crate::error::AppError> {
    // Update runtime state
    {
        let mut guard = language_store().write().expect("Failed to write language");
        *guard = lang;
    }

    // Persist to settings
    let mut settings = get_settings();
    settings.language = Some(lang.code().to_string());
    update_settings(settings)
}

/// Check if current language is Chinese
pub fn is_chinese() -> bool {
    current_language() == Language::Chinese
}

// ============================================================================
// Localized Text Macros and Functions
// ============================================================================

/// Get localized text based on current language
#[macro_export]
macro_rules! t {
    ($en:expr, $zh:expr) => {
        if $crate::cli::i18n::is_chinese() {
            $zh
        } else {
            $en
        }
    };
}

// Re-export for convenience
pub use t;

// ============================================================================
// Common UI Texts
// ============================================================================

pub mod texts;

#[cfg(test)]
mod tests {
    use super::{texts, use_test_language, Language};
    use std::sync::mpsc;
    use std::thread;

    #[test]
    fn website_url_label_keeps_optional_with_abbrev() {
        let label = texts::website_url_label();
        assert_eq!(label, "Website URL (opt.):");
        assert!(label.contains("(opt.)"));
        assert!(!label.contains("(optional)"));
    }

    #[test]
    fn chinese_tui_copy_avoids_key_mixed_english_labels() {
        let _lang = use_test_language(Language::Chinese);

        assert_eq!(texts::tui_home_section_connection(), "连接信息");
        assert_eq!(texts::tui_home_status_online(), "在线");
        assert_eq!(texts::tui_home_status_offline(), "离线");
        assert_eq!(texts::tui_label_mcp_servers_active(), "已启用");
        assert_eq!(texts::skills_management(), "技能管理");
        assert_eq!(texts::menu_manage_mcp(), "🔌 MCP 服务器");

        let help = texts::tui_help_text();
        assert!(help.contains("供应商：Enter 详情"));
        assert!(help.contains("供应商详情：s 切换"));
        assert!(help.contains("提示词：Enter 查看"));
        assert!(help.contains("技能：Enter 详情"));
        assert!(help.contains("配置：Enter 打开/执行"));
        assert!(help.contains("设置：Enter 应用"));
        assert!(!help.contains("Providers:"));
        assert!(!help.contains("Provider Detail:"));
        assert!(!help.contains("Skills:"));
        assert!(!help.contains("Config:"));
        assert!(!help.contains("Settings:"));
    }

    #[test]
    fn proxy_dashboard_copy_is_fully_localized_in_chinese() {
        let _lang = use_test_language(Language::Chinese);

        assert_eq!(texts::tui_home_section_connection(), "连接信息");
        assert_eq!(
            texts::tui_proxy_dashboard_failover_copy(),
            "仅做手动路由，不会自动切换供应商。"
        );
        assert_eq!(
            texts::tui_proxy_dashboard_manual_routing_copy("Claude"),
            "手动路由：Claude 的流量会通过 cc-switch。"
        );
    }

    #[test]
    fn test_language_override_does_not_leak_across_threads() {
        let _lang = use_test_language(Language::English);
        let (ready_tx, ready_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();

        let handle = thread::spawn(move || {
            let _lang = use_test_language(Language::Chinese);
            ready_tx.send(()).expect("signal ready");
            release_rx.recv().expect("wait for release");
        });

        ready_rx.recv().expect("wait for child language override");

        assert_eq!(
            texts::tui_home_section_connection(),
            "Connection Details",
            "child thread language override should not affect this test thread"
        );

        release_tx.send(()).expect("release child thread");
        handle.join().expect("join child thread");
    }
}
