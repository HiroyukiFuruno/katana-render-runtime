use super::{build_html_font_db, cjk_fallback_font_paths, load_font_paths, paths_for_font_family};
use crate::markdown::svg_rasterize::font::html::request::HtmlFontRequest;
use std::path::PathBuf;

#[test]
fn generic_serif_includes_linux_system_serif_candidates() {
    let paths = paths_for_font_family("serif");

    assert!(paths.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"));
    assert!(paths.contains(&"/usr/share/fonts/truetype/liberation2/LiberationSerif-Regular.ttf"));
    assert!(paths.contains(&"/usr/share/fonts/truetype/noto/NotoSerif-Regular.ttf"));
}

#[test]
fn generic_serif_cjk_uses_serif_fallback_candidates() {
    let request = HtmlFontRequest {
        families: vec!["serif".to_string()],
        needs_cjk: true,
        needs_emoji: false,
        needs_unicode_fallback: false,
    };
    let paths = cjk_fallback_font_paths(&request);

    assert!(paths.contains(&"/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc"));
    assert!(!paths.contains(&"/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"));
}

#[test]
fn mixed_generic_serif_and_sans_cjk_uses_both_fallback_candidates() {
    let request = HtmlFontRequest {
        families: vec!["sans-serif".to_string(), "serif".to_string()],
        needs_cjk: true,
        needs_emoji: false,
        needs_unicode_fallback: false,
    };
    let paths = cjk_fallback_font_paths(&request);

    assert!(paths.contains(&"/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc"));
    assert!(paths.contains(&"/usr/share/fonts/noto-cjk/NotoSansCJK-Regular.ttc"));
}

#[test]
fn named_installed_latin_families_use_selective_candidates() {
    let dejavu = paths_for_font_family("dejavu sans");
    let liberation = paths_for_font_family("liberation sans");

    assert!(dejavu.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"));
    assert!(
        liberation.contains(&"/usr/share/fonts/truetype/liberation2/LiberationSans-Regular.ttf")
    );
}

#[test]
fn courier_new_keeps_platform_exact_family_candidates_without_fontconfig() {
    let paths = paths_for_font_family("courier new");

    assert!(paths.contains(&"/System/Library/Fonts/Supplemental/Courier New.ttf"));
    assert!(paths.contains(&"C:/Windows/Fonts/cour.ttf"));
}

#[test]
fn generic_monospace_includes_bold_and_italic_candidates() {
    let paths = paths_for_font_family("monospace");

    assert!(paths.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf"));
    assert!(paths.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSansMono-Oblique.ttf"));
    assert!(paths.contains(&"/usr/share/fonts/truetype/dejavu/DejaVuSansMono-BoldOblique.ttf"));
}

#[test]
fn generic_cursive_and_fantasy_load_only_their_source_backed_candidates() {
    let cursive = paths_for_font_family("cursive");
    let fantasy = paths_for_font_family("fantasy");

    assert!(cursive.contains(&"/System/Library/Fonts/Supplemental/Apple Chancery.ttf"));
    assert!(cursive.contains(&"C:/Windows/Fonts/comic.ttf"));
    assert!(fantasy.contains(&"/System/Library/Fonts/Supplemental/Impact.ttf"));
    assert!(fantasy.contains(&"C:/Windows/Fonts/impact.ttf"));
    assert!(!cursive.iter().any(|path| fantasy.contains(path)));
}

#[test]
fn cjk_candidates_include_bold_faces_for_styled_fallback() {
    let request = HtmlFontRequest {
        families: vec!["sans-serif".to_string()],
        needs_cjk: true,
        needs_emoji: false,
        needs_unicode_fallback: false,
    };
    let paths = cjk_fallback_font_paths(&request);

    assert!(paths.contains(&"/usr/share/fonts/noto-cjk/NotoSansCJK-Bold.ttc"));
    assert!(paths.contains(&"C:/Windows/Fonts/YuGothB.ttc"));
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
