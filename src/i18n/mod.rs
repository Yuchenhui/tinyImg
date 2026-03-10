// rust-i18n 宏初始化
rust_i18n::i18n!("locales", fallback = "en");

/// 设置当前语言
pub fn set_language(lang: &str) {
    rust_i18n::set_locale(lang);
}
