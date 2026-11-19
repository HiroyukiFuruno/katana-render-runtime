use resvg::usvg::fontdb::{Database, Family, ID};
use std::{collections::BTreeSet, ops::ControlFlow, path::PathBuf};

#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) const INTERFACE_BOLD_WEIGHT: u16 = 700;

pub(in crate::markdown::svg_rasterize) trait PlatformFontStrategy {
    fn installed_paths(&self, families: &BTreeSet<&str>) -> BTreeSet<PathBuf>;

    fn installed_system_ui_family(&self) -> Option<&'static str> {
        None
    }

    fn system_alias(&self) -> Family<'static> {
        Family::SansSerif
    }

    fn face_override(
        &self,
        _: &Database,
        _: &str,
        _: &[String],
        _: u16,
        _: bool,
    ) -> ControlFlow<Option<ID>> {
        ControlFlow::Continue(())
    }
}

pub(in crate::markdown::svg_rasterize) fn font_platform() -> impl PlatformFontStrategy {
    #[cfg(any(windows, target_os = "macos", target_os = "ios"))]
    let selected = NativeFonts;
    #[cfg(not(any(windows, target_os = "macos", target_os = "ios")))]
    let selected = FontconfigFonts;
    selected
}

struct FontconfigFonts;

impl PlatformFontStrategy for FontconfigFonts {
    fn installed_paths(&self, families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
        super::fontconfig_font_paths(families)
    }

    #[cfg(target_os = "linux")]
    fn installed_system_ui_family(&self) -> Option<&'static str> {
        static FAMILY: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        FAMILY.get_or_init(fontconfig_system_ui_family).as_deref()
    }
}

#[cfg(target_os = "linux")]
fn fontconfig_system_ui_family() -> Option<String> {
    let output = crate::system::ProcessService::create_command("fc-match")
        .args(["--format", "%{family[0]}", "system-ui"])
        .output()
        .ok()?;
    let name = String::from_utf8(output.stdout).ok()?;
    let name = name.trim();
    (output.status.success() && !name.is_empty()).then(|| name.to_owned())
}

#[cfg(any(windows, target_os = "macos", target_os = "ios"))]
struct NativeFonts;

#[cfg(any(windows, target_os = "macos", target_os = "ios"))]
impl PlatformFontStrategy for NativeFonts {
    fn installed_paths(&self, families: &BTreeSet<&str>) -> BTreeSet<PathBuf> {
        let paths = super::native_system_font_paths(families);
        if paths.is_empty() {
            FontconfigFonts.installed_paths(families)
        } else {
            paths
        }
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    fn system_alias(&self) -> Family<'static> {
        Family::Name("System Font")
    }

    #[cfg(target_os = "macos")]
    fn face_override(
        &self,
        database: &Database,
        family: &str,
        names: &[String],
        weight: u16,
        italic: bool,
    ) -> ControlFlow<Option<ID>> {
        use super::super::sources::{bold_alias_id, is_internal_family};

        if is_internal_family(family) || names.iter().any(|name| is_internal_family(name)) {
            return ControlFlow::Break(None);
        }
        let family = family.trim().to_ascii_lowercase();
        if weight == INTERFACE_BOLD_WEIGHT
            && !italic
            && matches!(
                family.as_str(),
                ".hiragino kaku gothic interface" | "hiragino kaku gothic interface"
            )
            && let Some(selected) = bold_alias_id(database)
        {
            return ControlFlow::Break(Some(selected));
        }
        ControlFlow::Continue(())
    }
}
