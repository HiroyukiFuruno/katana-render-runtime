use super::super::tests::FONT_BYTES;
use super::same_fallback_attributes;
use resvg::usvg::fontdb::{Database, FaceInfo, Stretch, Style, Weight};

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn browser_system_font_aliases_select_the_registered_system_face()
-> Result<(), Box<dyn std::error::Error>> {
    let database = system_alias_database()?;
    for (family, style) in [
        ("system-ui", "normal"),
        ("-apple-system", "italic"),
        ("BlinkMacSystemFont", "oblique"),
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="{family}" font-style="{style}">AV</text></svg>"#
        );
        let mut options = resvg::usvg::Options {
            fontdb: std::sync::Arc::clone(&database),
            ..resvg::usvg::Options::default()
        };
        options.font_resolver.select_font = super::html_font_selector();
        let tree = resvg::usvg::Tree::from_str(&svg, &options)?;
        assert!(!tree.root().children().is_empty(), "{family}/{style}");
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn loaded_selector_defers_system_aliases_even_with_a_registered_face() -> Result<(), String> {
    let database = system_alias_database()?;
    for family in ["system-ui", "-apple-system", "BlinkMacSystemFont"] {
        let font = requested_svg_font(&database, family)?;
        assert_eq!(super::loaded_html_font(&font, &database), None, "{family}");
        let mut selected_database = std::sync::Arc::clone(&database);
        assert!(
            super::html_font_selector()(&font, &mut selected_database).is_some(),
            "{family}"
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn internal_interface_family_is_not_a_user_selectable_font() -> Result<(), String> {
    const INTERNAL_INTERFACE_BOLD_FAMILY: &str = "__krr_internal_system_ui_interface_bold_700";
    let mut database = Database::new();
    let mut face = bundled_face()?;
    face.families = vec![(
        INTERNAL_INTERFACE_BOLD_FAMILY.to_owned(),
        resvg::usvg::fontdb::Language::English_UnitedStates,
    )];
    database.push_face_info(face);
    let database = std::sync::Arc::new(database);
    let font = requested_svg_font(&database, INTERNAL_INTERFACE_BOLD_FAMILY)?;
    assert_eq!(super::loaded_html_font(&font, &database), None);
    let mut selected_database = std::sync::Arc::clone(&database);
    assert_eq!(
        super::html_font_selector()(&font, &mut selected_database),
        None
    );
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn requested_svg_font(
    database: &std::sync::Arc<Database>,
    family: &str,
) -> Result<resvg::usvg::Font, String> {
    let id = database.faces().next().ok_or("fixture face missing")?.id;
    let mut options = resvg::usvg::Options {
        fontdb: std::sync::Arc::clone(database),
        ..resvg::usvg::Options::default()
    };
    options.font_resolver.select_font = Box::new(move |_, _| Some(id));
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><text id="run" y="20" font-family="{family}">AV</text></svg>"#
    );
    let tree = resvg::usvg::Tree::from_str(&svg, &options).map_err(|error| error.to_string())?;
    let Some(resvg::usvg::Node::Text(text)) = tree.node_by_id("run") else {
        return Err("fixture text missing".into());
    };
    let font = text
        .chunks()
        .first()
        .and_then(|chunk| chunk.spans().first())
        .map(|span| span.font().clone())
        .ok_or("fixture font missing")?;
    assert!(
        matches!(font.families().first(), Some(resvg::usvg::FontFamily::Named(name)) if name.eq_ignore_ascii_case(family))
    );
    Ok(font)
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn system_alias_database() -> Result<std::sync::Arc<Database>, String> {
    let mut database = Database::new();
    for style in [Style::Normal, Style::Italic, Style::Oblique] {
        let mut face = bundled_face()?;
        face.style = style;
        face.families = vec![(
            "System Font".to_string(),
            resvg::usvg::fontdb::Language::English_UnitedStates,
        )];
        database.push_face_info(face);
    }
    Ok(std::sync::Arc::new(database))
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
#[test]
fn system_alias_preserves_author_family_order_and_stretch() -> Result<(), Box<dyn std::error::Error>>
{
    let mut database = system_alias_database()?;
    std::sync::Arc::make_mut(&mut database).load_font_data(FONT_BYTES.to_vec());
    for stretch in [
        "ultra-condensed",
        "extra-condensed",
        "condensed",
        "semi-condensed",
        "normal",
        "semi-expanded",
        "expanded",
        "extra-expanded",
        "ultra-expanded",
    ] {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text y="20" font-family="Noto Sans,serif,sans-serif,cursive,fantasy,monospace,system-ui" font-stretch="{stretch}">AV</text></svg>"#
        );
        let mut options = resvg::usvg::Options {
            fontdb: std::sync::Arc::clone(&database),
            ..resvg::usvg::Options::default()
        };
        options.font_resolver.select_font = super::html_font_selector();
        let tree = resvg::usvg::Tree::from_str(&svg, &options)?;
        assert!(!tree.root().children().is_empty(), "{stretch}");
        assert_selected_family(&tree, "Noto Sans")?;
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "ios"))]
fn assert_selected_family(tree: &resvg::usvg::Tree, expected: &str) -> Result<(), String> {
    let mut count = 0;
    for node in tree.root().children() {
        let resvg::usvg::Node::Text(text) = node else {
            continue;
        };
        for span in text.layouted() {
            for glyph in &span.positioned_glyphs {
                let face = tree
                    .fontdb()
                    .face(glyph.font)
                    .ok_or("selected font missing")?;
                assert!(face.families.iter().any(|(name, _)| name == expected));
                count += 1;
            }
        }
    }
    assert!(count > 0);
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn mixed_face_tspans_keep_latin_on_the_requested_face() -> Result<(), Box<dyn std::error::Error>> {
    let family = "Helvetica Neue, Hiragino Kaku Gothic ProN";
    let database = super::super::super::html_font_db_for_text(family, "AV日本");
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="30"><text x="0" y="20" font-size="16"><tspan x="0" font-family="Helvetica Neue">AV</tspan><tspan x="22" font-family="Hiragino Kaku Gothic ProN">日本</tspan></text></svg>"#;
    let options = resvg::usvg::Options {
        fontdb: database,
        ..resvg::usvg::Options::default()
    };
    let tree = resvg::usvg::Tree::from_str(svg, &options)?;
    for node in tree.root().children() {
        let resvg::usvg::Node::Text(text) = node else {
            continue;
        };
        for span in text.layouted() {
            for glyph in &span.positioned_glyphs {
                assert_latin_face(&tree, glyph.font, &glyph.text)?;
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn assert_latin_face(
    tree: &resvg::usvg::Tree,
    font: resvg::usvg::fontdb::ID,
    text: &str,
) -> Result<(), String> {
    if !text.is_ascii() {
        return Ok(());
    }
    let face = tree.fontdb().face(font).ok_or("font missing")?;
    assert!(
        face.families
            .iter()
            .any(|(name, _)| name == "Helvetica Neue"),
        "{text} {:?}",
        face.families
    );
    Ok(())
}

fn bundled_face() -> Result<FaceInfo, String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let mut face = database
        .faces()
        .next()
        .ok_or_else(|| "bundled font should contain a face".to_string())?
        .clone();
    face.style = Style::Normal;
    face.weight = Weight::NORMAL;
    face.stretch = Stretch::Normal;
    Ok(face)
}

fn face_with_attributes(
    base: &FaceInfo,
    style: Style,
    weight: Weight,
    stretch: Stretch,
) -> FaceInfo {
    let mut face = base.clone();
    face.style = style;
    face.weight = weight;
    face.stretch = stretch;
    face
}

#[test]
fn fallback_selection_rejects_a_base_face_missing_from_the_database() {
    let database = std::sync::Arc::new(Database::new());
    assert_eq!(
        super::select_cached('A', &[resvg::usvg::fontdb::ID::dummy()], &database),
        Ok(None)
    );
}

#[test]
fn fallback_probe_rejects_an_excluded_real_font_face() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(FONT_BYTES.to_vec());
    let face = database
        .faces()
        .next()
        .cloned()
        .ok_or("font face missing")?;
    let id = face.id;
    let database = std::sync::Arc::new(database);
    assert_eq!(
        super::fallback_probe(&database, &face, &[], id, 'A'),
        Some(true)
    );
    assert_eq!(
        super::fallback_probe(&database, &face, &[id], id, 'A'),
        Some(false)
    );
    Ok(())
}

#[test]
fn fallback_attribute_comparison_preserves_usvg_conjunction_semantics() -> Result<(), String> {
    let base = bundled_face()?;

    let same = face_with_attributes(&base, Style::Normal, Weight::NORMAL, Stretch::Normal);
    assert!(same_fallback_attributes(&base, &same));

    let style_only = face_with_attributes(&base, Style::Italic, Weight::NORMAL, Stretch::Normal);
    assert!(same_fallback_attributes(&base, &style_only));

    let style_and_weight =
        face_with_attributes(&base, Style::Italic, Weight::BOLD, Stretch::Normal);
    assert!(same_fallback_attributes(&base, &style_and_weight));

    let all_differ = face_with_attributes(&base, Style::Italic, Weight::BOLD, Stretch::Condensed);
    assert!(!same_fallback_attributes(&base, &all_differ));
    Ok(())
}
