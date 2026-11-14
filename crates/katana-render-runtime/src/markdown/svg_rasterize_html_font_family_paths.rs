use super::paths::{
    ARIAL_FONT_PATHS, CJK_FONT_PATHS, CJK_SERIF_FONT_PATHS, CURSIVE_FONT_PATHS,
    DEJAVU_SANS_FONT_PATHS, EMOJI_FONT_PATHS, FANTASY_FONT_PATHS, HELVETICA_FONT_PATHS,
    LIBERATION_SANS_FONT_PATHS, MONOSPACE_FONT_PATHS, SYSTEM_CJK_FONT_PATHS, SYSTEM_FONT_PATHS,
    TIMES_FONT_PATHS, TREBUCHET_FONT_PATHS,
};

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
