use super::super::{BUNDLED_SANS_SERIF_FONT, configure_generic_families};
use super::request::HtmlFontRequest;
use super::system::load_requested_system_font_family;
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
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-Italic.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSerif-BoldItalic.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSerif-Regular.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSerif-Bold.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSerif-Italic.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSerif-BoldItalic.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-Italic.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSerif-BoldItalic.ttf",
    "/usr/share/fonts/truetype/noto/NotoSerif-Regular.ttf",
    "/usr/share/fonts/truetype/noto/NotoSerif-Bold.ttf",
    "/usr/share/fonts/truetype/noto/NotoSerif-Italic.ttf",
    "/usr/share/fonts/opentype/urw-base35/NimbusRoman-Regular.otf",
    "/usr/share/fonts/opentype/urw-base35/NimbusRoman-Bold.otf",
    "/usr/share/fonts/opentype/urw-base35/NimbusRoman-Italic.otf",
    "/usr/share/fonts/truetype/freefont/FreeSerif.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSerifBold.ttf",
    "/usr/share/fonts/truetype/freefont/FreeSerifItalic.ttf",
    "C:/Windows/Fonts/times.ttf",
    "C:/Windows/Fonts/timesbd.ttf",
];
const DEJAVU_SANS_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-Oblique.ttf",
    "/usr/share/fonts/truetype/dejavu/DejaVuSans-BoldOblique.ttf",
];
const LIBERATION_SANS_FONT_PATHS: &[&str] = &[
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-Italic.ttf",
    "/usr/share/fonts/truetype/liberation2/LiberationSans-BoldItalic.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-Italic.ttf",
    "/usr/share/fonts/truetype/liberation/LiberationSans-BoldItalic.ttf",
];
const CJK_SERIF_FONT_PATHS: &[&str] = &[
    "/System/Library/Fonts/Hiragino Mincho ProN.ttc",
    "/System/Library/Fonts/Hiragino Mincho Pro.ttc",
    "/System/Library/Fonts/ヒラギノ明朝 ProN.ttc",
    "/System/Library/Fonts/Supplemental/Yu Mincho.ttc",
    "/usr/share/fonts/opentype/noto/NotoSerifCJK-Regular.ttc",
    "/usr/share/fonts/truetype/noto/NotoSerifCJK-Regular.ttc",
    "/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc",
    "C:/Windows/Fonts/YuMincho.ttc",
    "C:/Windows/Fonts/msmincho.ttc",
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
        let paths = paths_for_font_family(family);
        if load_font_paths(&mut database, paths) == 0 {
            load_requested_system_font_family(&mut database, family);
        }
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

fn load_font_paths(database: &mut usvg::fontdb::Database, paths: &[&str]) -> usize {
    let mut loaded = 0;
    for path in paths {
        if Path::new(path).is_file() {
            database.load_font_source(usvg::fontdb::Source::File((*path).into()));
            loaded += 1;
        }
    }
    loaded
}

fn paths_for_font_family(family: &str) -> &'static [&'static str] {
    match family {
        "arial" | "arial unicode ms" => ARIAL_FONT_PATHS,
        "dejavu sans" => DEJAVU_SANS_FONT_PATHS,
        "helvetica" | "helvetica neue" | "system-ui" => HELVETICA_FONT_PATHS,
        "liberation sans" => LIBERATION_SANS_FONT_PATHS,
        "times" | "times new roman" | "serif" => TIMES_FONT_PATHS,
        "menlo" | "consolas" | "courier" | "courier new" | "monospace" => MONOSPACE_FONT_PATHS,
        "hiragino sans" | "hiragino sans gb" | "yu gothic" | "meiryo" | "noto sans jp"
        | "noto sans cjk jp" => CJK_FONT_PATHS,
        "noto serif jp" | "noto serif cjk jp" | "yu mincho" => CJK_SERIF_FONT_PATHS,
        "apple color emoji" | "segoe ui emoji" | "noto color emoji" => EMOJI_FONT_PATHS,
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::{build_html_font_db, load_font_paths, paths_for_font_family};
    use crate::markdown::svg_rasterize::font::html::request::HtmlFontRequest;
    use std::path::PathBuf;

    #[test]
    fn generic_serif_includes_linux_system_serif_candidates() {
        let paths = paths_for_font_family("serif");

        assert!(paths.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"));
        assert!(
            paths.contains(&"/usr/share/fonts/truetype/liberation2/LiberationSerif-Regular.ttf")
        );
        assert!(paths.contains(&"/usr/share/fonts/truetype/noto/NotoSerif-Regular.ttf"));
    }

    #[test]
    fn named_installed_latin_families_use_selective_candidates() {
        let dejavu = paths_for_font_family("dejavu sans");
        let liberation = paths_for_font_family("liberation sans");

        assert!(dejavu.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"));
        assert!(
            liberation
                .contains(&"/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf")
        );
    }

    #[test]
    fn named_serif_cjk_families_use_serif_candidates() {
        for family in ["noto serif jp", "noto serif cjk jp", "yu mincho"] {
            let paths = paths_for_font_family(family);
            assert!(paths.contains(&"/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc"));
            assert!(!paths.contains(&"/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"));
        }
    }

    #[test]
    fn load_font_paths_registers_existing_files_only() -> Result<(), std::io::Error> {
        let existing = std::env::current_exe()?;
        let existing = existing.to_string_lossy().into_owned();
        let missing = PathBuf::from("/krr-font-path-that-does-not-exist.ttf");
        let missing = missing.to_string_lossy().into_owned();
        let paths = [existing.as_str(), missing.as_str()];
        let mut database = resvg::usvg::fontdb::Database::new();

        assert_eq!(load_font_paths(&mut database, &paths), 1);
        Ok(())
    }

    #[test]
    fn build_html_font_db_exercises_all_optional_source_requests() {
        let request = HtmlFontRequest {
            families: vec!["krr-family-that-cannot-exist-7f0b".to_string()],
            needs_cjk: true,
            needs_emoji: true,
            needs_unicode_fallback: true,
        };

        let database = build_html_font_db(&request);
        assert!(database.faces().count() >= 1);
    }
}
