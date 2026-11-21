use super::PlatformFontStrategy;
use std::{collections::BTreeSet, path::PathBuf};

pub(super) struct FontconfigFonts;

impl PlatformFontStrategy for FontconfigFonts {
    fn installed_paths(&self, families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
        super::super::fontconfig_font_paths(families)
    }

    #[cfg(target_os = "linux")]
    fn control_arial_family(&self) -> &'static str {
        "Arial, sans-serif"
    }

    #[cfg(target_os = "linux")]
    fn compatible_named_family(&self, requested: &str) -> Option<&'static str> {
        /* WHY: FontconfigのArial互換書体を日本語用の汎用書体より先に試し、明示された書体の優先順を保つ。 */
        requested
            .eq_ignore_ascii_case("Arial")
            .then_some("Liberation Sans")
    }

    #[cfg(target_os = "linux")]
    fn installed_system_ui_family(&self) -> Option<&'static str> {
        static FAMILY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        FAMILY
            .get_or_init(|| fontconfig_matched_family("system-ui"))
            .as_deref()
    }

    #[cfg(target_os = "linux")]
    fn cjk_fallback_family(&self) -> Option<&'static str> {
        static FAMILY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        /* WHY: ひらがな・カタカナ・漢字を扱えるOS標準の候補を一度だけ取得し、個々の字形は既存の検査で確認する。 */
        const CJK_SCALABLE_PATTERN: &str = ":scalable=true:charset=3042 30a2 4e00";
        FAMILY
            .get_or_init(|| fontconfig_matched_family(CJK_SCALABLE_PATTERN))
            .as_deref()
    }

    #[cfg(target_os = "linux")]
    fn localized_sans_serif(&self, language: &str) -> Option<&'static str> {
        let primary = language.split('-').next()?;
        /* WHY: Linux Chromeの日本語sans-serifは、欧文を含めて日本語用の書体リストを使う。 */
        primary
            .eq_ignore_ascii_case("ja")
            .then_some("Noto Sans JP, Noto Sans CJK JP, sans-serif")
    }
}

#[cfg(target_os = "linux")]
fn fontconfig_matched_family(pattern: &str) -> Option<String> {
    let output = crate::system::ProcessService::create_command("fc-match")
        .args(["--format", "%{family[0]}", pattern])
        .output()
        .ok()?;
    let name = String::from_utf8(output.stdout).ok()?;
    let name = name.trim();
    (output.status.success() && !name.is_empty()).then(|| name.to_owned())
}
