use super::super::{BUNDLED_SANS_SERIF_FONT, configure_generic_families};
use super::request::HtmlFontRequest;
use resvg::usvg;
use std::{
    path::Path,
    sync::{Arc, OnceLock},
};

const CJK_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/ヒラギノ角ゴシック W3.ttc",
    "/System/Library/Fonts/Hiragino Sans GB.ttc",
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc",
    "C:/Windows/Fonts/YuGothR.ttc",
    "C:/Windows/Fonts/meiryo.ttc",
];
const EMOJI_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Apple Color Emoji.ttc",
    "/usr/share/fonts/truetype/noto/NotoColorEmoji.ttf",
    "/usr/share/fonts/google-noto-emoji/NotoColorEmoji.ttf",
    "C:/Windows/Fonts/seguiemj.ttf",
];
const UNICODE_FALLBACK_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
    "C:/Windows/Fonts/arialuni.ttf",
];
const HELVETICA_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Helvetica.ttc",
    "/System/Library/Fonts/HelveticaNeue.ttc",
];
const ARIAL_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Supplemental/Arial.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold.ttf",
    "/System/Library/Fonts/Supplemental/Arial Italic.ttf",
    "/System/Library/Fonts/Supplemental/Arial Bold Italic.ttf",
    "C:/Windows/Fonts/arial.ttf",
    "C:/Windows/Fonts/arialbd.ttf",
    "C:/Windows/Fonts/ariali.ttf",
    "C:/Windows/Fonts/arialbi.ttf",
];
const TIMES_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Times.ttc",
    "/System/Library/Fonts/Supplemental/Times New Roman.ttf",
    "C:/Windows/Fonts/times.ttf",
    "C:/Windows/Fonts/timesbd.ttf",
];
const MONOSPACE_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Menlo.ttc",
    "/System/Library/Fonts/Courier.ttc",
    "/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf",
    "C:/Windows/Fonts/consola.ttf",
];

pub(super) fn build_html_font_db(request: &HtmlFontRequest) -> usvg::fontdb::Database {
    let mut database = usvg::fontdb::Database::new();
    /* WHY: bundled Latin は一度だけ共有し、system font は必要な file source だけを登録する。 */
    database.load_font_source(bundled_font_source());
    for family in &request.families {
        load_font_paths(&mut database, paths_for_font_family(family));
    }
    if request.needs_cjk {
        load_font_paths(&mut database, CJK_FONT_PATHS);
    }
    if request.needs_emoji {
        load_font_paths(&mut database, EMOJI_FONT_PATHS);
    }
    if request.needs_unicode_fallback {
        load_font_paths(&mut database, UNICODE_FALLBACK_FONT_PATHS);
    }
    configure_generic_families(&mut database);
    database
}

fn bundled_font_source() -> usvg::fontdb::Source {
    static BUNDLED_FONT_DATA: OnceLock<Arc<dyn AsRef<[u8]> + Send + Sync>> = OnceLock::new();
    usvg::fontdb::Source::Binary(Arc::clone(
        BUNDLED_FONT_DATA.get_or_init(|| Arc::new(BUNDLED_SANS_SERIF_FONT.to_vec())),
    ))
}

fn load_font_paths(database: &mut usvg::fontdb::Database, paths: &[&str]) {
    for path in paths {
        if Path::new(path).is_file() {
            database.load_font_source(usvg::fontdb::Source::File((*path).into()));
        }
    }
}

fn paths_for_font_family(family: &str) -> &'static [&'static str] {
    match family {
        "arial" | "arial unicode ms" => ARIAL_FONT_PATHS,
        "helvetica" | "helvetica neue" | "system-ui" => HELVETICA_FONT_PATHS,
        "times" | "times new roman" | "serif" => TIMES_FONT_PATHS,
        "menlo" | "consolas" | "courier" | "courier new" | "monospace" => MONOSPACE_FONT_PATHS,
        "hiragino sans" | "hiragino sans gb" | "yu gothic" | "meiryo" | "noto sans jp"
        | "noto sans cjk jp" | "noto serif jp" | "noto serif cjk jp" | "yu mincho" => {
            CJK_FONT_PATHS
        }
        "apple color emoji" | "segoe ui emoji" | "noto color emoji" => EMOJI_FONT_PATHS,
        _ => &[],
    }
}
