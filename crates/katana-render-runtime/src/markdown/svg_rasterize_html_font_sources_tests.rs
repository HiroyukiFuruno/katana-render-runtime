use super::{
    build_html_font_db, canonical_font_source_path, cjk_fallback_font_paths, load_font_paths,
    load_font_paths_once, paths_for_font_family,
};
use crate::markdown::svg_rasterize::font::BUNDLED_SANS_SERIF_FONT;
use crate::markdown::svg_rasterize::font::html::request::HtmlFontRequest;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
#[test]
fn system_ui_cjk_uses_the_native_interface_face() -> Result<(), String> {
    use crate::markdown::svg_rasterize::SvgRasterizeOps;
    let runs = SvgRasterizeOps::html_text_font_runs("名称", "system-ui", 400, false)
        .ok_or("system UI font runs are missing")?;
    assert_eq!(
        runs,
        vec![(
            ".Hiragino Kaku Gothic Interface".to_string(),
            "名称".to_string()
        )]
    );
    let width =
        SvgRasterizeOps::measure_html_text("名称", "system-ui", 16.0, 400, false, 0.0, None)
            .map_err(|error| format!("{error:?}"))?;
    assert!((width - 32000.0 / 1083.0).abs() < 0.001);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn system_cjk_fallback_preserves_explicit_author_family() -> Result<(), String> {
    use crate::markdown::svg_rasterize::SvgRasterizeOps;
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    let width = SvgRasterizeOps::measure_html_text("名称", family, 16.0, 400, false, 0.0, None)
        .map_err(|error| format!("{error:?}"))?;
    assert!((width - 32.0).abs() < 0.001);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn html_serif_uses_browser_latin_family_before_cjk_fallback() -> Result<(), String> {
    use resvg::usvg::fontdb::{Family, Query};
    let database = build_html_font_db(&HtmlFontRequest {
        families: vec!["serif".to_string()],
        needs_cjk: true,
        needs_emoji: false,
        needs_unicode_fallback: false,
    });
    let browser_family = database
        .query(&Query {
            families: &[Family::Name("Times")],
            ..Query::default()
        })
        .ok_or_else(|| "macOS browser serif font is missing".to_string())?;
    assert_eq!(
        database.query(&Query {
            families: &[Family::Serif],
            ..Query::default()
        }),
        Some(browser_family)
    );
    Ok(())
}

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
fn system_ui_cjk_preserves_author_serif_fallback_sources() {
    let request = HtmlFontRequest {
        families: vec!["system-ui".to_string(), "serif".to_string()],
        needs_cjk: true,
        needs_emoji: false,
        needs_unicode_fallback: false,
    };
    let paths = cjk_fallback_font_paths(&request);
    assert!(paths.contains(&"/System/Library/Fonts/ヒラギノ角ゴシック W4.ttc"));
    assert!(paths.contains(&"/usr/share/fonts/noto-cjk/NotoSerifCJK-Regular.ttc"));
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
fn macos_system_aliases_load_the_system_font_source() {
    for family in [
        "system-ui",
        "-apple-system",
        "blinkmacsystemfont",
        "system font",
        ".sf ns",
    ] {
        assert_eq!(
            paths_for_font_family(family),
            &["/System/Library/Fonts/SFNS.ttf"]
        );
    }
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
    assert!(cursive.contains(&"/usr/share/fonts/opentype/urw-base35/Z003-MediumItalic.otf"));
    assert!(cursive.contains(&"C:/Windows/Fonts/comic.ttf"));
    assert!(cursive.contains(&"C:/Windows/Fonts/comicbd.ttf"));
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
fn load_font_paths_once_deduplicates_one_physical_source_with_aliases() -> Result<(), std::io::Error>
{
    let directory = tempfile::tempdir()?;
    let subdirectory = directory.path().join("sub");
    std::fs::create_dir(&subdirectory)?;
    let font_path = directory.path().join("font.ttf");
    std::fs::write(&font_path, BUNDLED_SANS_SERIF_FONT)?;
    let alias = subdirectory.join("..").join("font.ttf");
    let font_path = font_path.to_string_lossy().into_owned();
    let alias = alias.to_string_lossy().into_owned();
    let paths = [font_path.as_str(), alias.as_str()];
    let mut database = resvg::usvg::fontdb::Database::new();
    let mut loaded = std::collections::BTreeSet::new();

    assert_eq!(load_font_paths_once(&mut database, &paths, &mut loaded), 1);
    assert_eq!(loaded.len(), 1);
    assert_eq!(database.faces().count(), 1);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn html_database_loads_each_system_file_source_once() -> Result<(), std::io::Error> {
    let database = build_html_font_db(&HtmlFontRequest {
        families: vec!["Helvetica Neue".to_string(), "sans-serif".to_string()],
        needs_cjk: false,
        needs_emoji: false,
        needs_unicode_fallback: false,
    });
    let mut unique = std::collections::BTreeSet::new();
    for face in database.faces() {
        if let resvg::usvg::fontdb::Source::File(path) = &face.source {
            assert!(unique.insert((canonical_font_source_path(path)?, face.index)));
        }
    }
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

#[cfg(target_os = "macos")]
#[test]
fn macos_hiragino_faces_use_chrome_css_weight_mapping() -> Result<(), String> {
    let request = HtmlFontRequest {
        families: vec!["Hiragino Sans".to_string()],
        needs_cjk: true,
        needs_emoji: false,
        needs_unicode_fallback: false,
    };
    let database = build_html_font_db(&request);
    for (post_script_name, expected) in [("HiraginoSans-W3", 400), ("HiraginoSans-W6", 700)] {
        let face = database
            .faces()
            .find(|face| face.post_script_name == post_script_name)
            .ok_or_else(|| format!("missing {post_script_name}"))?;
        assert_eq!(face.weight.0, expected, "{post_script_name}");
    }
    Ok(())
}
