use super::paths::{
    ARIAL_FONT_PATHS, CJK_FONT_PATHS, CJK_SERIF_FONT_PATHS, CURSIVE_FONT_PATHS,
    DEJAVU_SANS_FONT_PATHS, EMOJI_FONT_PATHS, FANTASY_FONT_PATHS, HELVETICA_FONT_PATHS,
    LIBERATION_SANS_FONT_PATHS, MONOSPACE_FONT_PATHS, SYSTEM_CJK_FONT_PATHS, SYSTEM_FONT_PATHS,
    TIMES_FONT_PATHS, TREBUCHET_FONT_PATHS,
};
use super::{configured_html_sans_serif_family, load_font_paths_once};
use resvg::usvg;
use std::{collections::BTreeSet, path::PathBuf};

pub(super) fn load_requested_font_family(
    database: &mut usvg::fontdb::Database,
    family: &str,
    loaded_paths: &mut BTreeSet<PathBuf>,
) {
    #[cfg(target_os = "linux")]
    if family == "monospace" {
        /* WHY: 日本語文中のASCII断片でも、描画全体と同じ等幅書体を候補にする。 */
        load_font_paths_once(database, CJK_FONT_PATHS, loaded_paths);
    }
    if family == "sans-serif" {
        let configured = configured_html_sans_serif_family().to_ascii_lowercase();
        load_font_paths_once(database, paths_for_font_family(&configured), loaded_paths);
        load_font_paths_once(database, ARIAL_FONT_PATHS, loaded_paths);
        load_font_paths_once(database, LIBERATION_SANS_FONT_PATHS, loaded_paths);
    } else {
        load_font_paths_once(database, paths_for_font_family(family), loaded_paths);
    }
}

pub(super) fn paths_for_font_family(family: &str) -> &'static [&'static str] {
    match family {
        "arial" | "arial unicode ms" => ARIAL_FONT_PATHS,
        "dejavu sans" => DEJAVU_SANS_FONT_PATHS,
        "helvetica" | "helvetica neue" => HELVETICA_FONT_PATHS,
        "system-ui" | "-apple-system" | "blinkmacsystemfont" | ".sf ns" | "system font" => {
            SYSTEM_FONT_PATHS
        }
        ".hiragino kaku gothic interface" => SYSTEM_CJK_FONT_PATHS,
        "liberation sans" => LIBERATION_SANS_FONT_PATHS,
        "trebuchet ms" => TREBUCHET_FONT_PATHS,
        "times" | "times new roman" | "serif" => TIMES_FONT_PATHS,
        "menlo" | "consolas" | "courier" | "courier new" | "monospace" => MONOSPACE_FONT_PATHS,
        "cursive" => CURSIVE_FONT_PATHS,
        "fantasy" => FANTASY_FONT_PATHS,
        "hiragino sans" | "hiragino sans gb" | "yu gothic" | "meiryo" | "noto sans jp"
        | "noto sans cjk jp" => CJK_FONT_PATHS,
        "noto serif jp" | "noto serif cjk jp" | "yu mincho" => CJK_SERIF_FONT_PATHS,
        "apple color emoji" | "segoe ui emoji" | "noto color emoji" => EMOJI_FONT_PATHS,
        _ => &[],
    }
}
