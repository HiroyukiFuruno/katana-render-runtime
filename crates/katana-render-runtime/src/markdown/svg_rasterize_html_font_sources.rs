use super::request::HtmlFontRequest;
use resvg::usvg;
use std::collections::BTreeSet;

#[path = "svg_rasterize_html_generic_families.rs"]
mod generic_families;
#[path = "svg_rasterize_html_font_paths.rs"]
mod paths;
#[path = "svg_rasterize_html_font_source_identity.rs"]
mod source_identity;
use generic_families::configure_html_generic_families;
pub(super) use generic_families::configured_html_sans_serif_family;
use paths::{
    ARIAL_FONT_PATHS, CJK_FONT_PATHS, CJK_SERIF_FONT_PATHS, CURSIVE_FONT_PATHS,
    DEJAVU_SANS_FONT_PATHS, EMOJI_FONT_PATHS, FANTASY_FONT_PATHS, HELVETICA_FONT_PATHS,
    LIBERATION_SANS_FONT_PATHS, MONOSPACE_FONT_PATHS, SYSTEM_CJK_FONT_PATHS, SYSTEM_FONT_PATHS,
    TIMES_FONT_PATHS, TREBUCHET_FONT_PATHS, UNICODE_FALLBACK_FONT_PATHS,
};
use source_identity::bundled_font_source;
pub(super) use source_identity::canonical_font_source_path;
#[cfg(test)]
pub(super) use source_identity::load_font_paths;
pub(super) use source_identity::load_font_paths_once;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_known_builder.rs"]
mod known_builder;
#[cfg(all(target_os = "macos", test))]
pub(super) use known_builder::build as build_known_html_font_db;
#[cfg(all(target_os = "macos", test))]
pub(super) use known_builder::build_with_shared_budget as build_known_html_font_db_with_shared_budget;
#[cfg(target_os = "macos")]
pub(super) use known_builder::build_with_shared_sources as build_known_html_font_db_with_shared_sources;

#[cfg(target_os = "macos")]
#[path = "svg_rasterize_html_font_interface_bold.rs"]
mod interface_bold;
#[cfg(target_os = "macos")]
pub(in crate::markdown::svg_rasterize) use interface_bold::{
    bold_alias_id, has_native_interface_candidate, is_interface_face, is_internal_family,
    is_system_ui_primary, register_interface_bold,
};

#[cfg(target_os = "macos")]
const MAX_HIRAGINO_WEIGHT_INDEX: u16 = 9;

/* WHY: macOSのブラウザ別名を実System Fontへ解決し、SVGと計測のfamily選択を揃える。 */
pub(super) fn build_html_font_db(request: &HtmlFontRequest) -> usvg::fontdb::Database {
    let mut database = usvg::fontdb::Database::new();
    let mut loaded_paths = BTreeSet::new();
    /* WHY: bundled Latin は一度だけ共有し、system font は必要な file source だけを登録する。 */
    database.load_font_source(bundled_font_source());
    for family in &request.families {
        load_requested_font_family(&mut database, family, &mut loaded_paths);
    }
    super::system::load_requested_system_font_families_once(
        &mut database,
        request
            .families
            .iter()
            .map(String::as_str)
            .filter(|family| !is_generic_font_family(family)),
        &mut loaded_paths,
    );
    load_fallback_font_paths(request, &mut database, &mut loaded_paths);
    /* WHY: ChromeのmacOS CSS weightはHiraginoのW番号をOS/2 weightと別に解釈する。 */
    #[cfg(target_os = "macos")]
    {
        normalize_macos_hiragino_weights(&mut database);
        if interface_bold_cjk_request(request) {
            let _ = register_interface_bold(&mut database);
        }
    }
    configure_html_generic_families(&mut database);
    database
}

#[cfg(target_os = "macos")]
fn normalize_macos_hiragino_weights(database: &mut usvg::fontdb::Database) {
    let faces = database
        .faces()
        .filter_map(|face| {
            let weight = hiragino_css_weight(&face.post_script_name)?;
            let is_hiragino = face
                .families
                .iter()
                .any(|(family, _)| family == "Hiragino Sans");
            let is_native = match &face.source {
                usvg::fontdb::Source::File(path) | usvg::fontdb::Source::SharedFile(path, _) => {
                    path.to_string_lossy().starts_with("/System/Library/Fonts/")
                }
                _ => false,
            };
            if is_hiragino && is_native && face.weight.0 != weight {
                Some((face.clone(), weight))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    for (mut face, weight) in faces {
        let id = face.id;
        face.weight = usvg::fontdb::Weight(weight);
        database.remove_face(id);
        database.push_face_info(face);
    }
}

fn interface_bold_cjk_request(request: &HtmlFontRequest) -> bool {
    request.needs_cjk
        && request.families.iter().any(|family| {
            matches!(
                family.as_str(),
                "system-ui"
                    | "-apple-system"
                    | "blinkmacsystemfont"
                    | ".hiragino kaku gothic interface"
            )
        })
}

#[cfg(target_os = "macos")]
fn hiragino_css_weight(post_script_name: &str) -> Option<u16> {
    let suffix = post_script_name.strip_prefix("HiraginoSans-W")?;
    let index = suffix.parse::<u16>().ok()?;
    (index <= MAX_HIRAGINO_WEIGHT_INDEX).then_some((index + 1) * 100)
}

fn load_fallback_font_paths(
    request: &HtmlFontRequest,
    database: &mut usvg::fontdb::Database,
    loaded_paths: &mut BTreeSet<std::path::PathBuf>,
) {
    if request.needs_cjk {
        let paths = cjk_fallback_font_paths(request);
        load_font_paths_once(database, &paths, loaded_paths);
    }
    if request.needs_emoji {
        load_font_paths_once(database, EMOJI_FONT_PATHS, loaded_paths);
    }
    if request.needs_unicode_fallback {
        load_font_paths_once(database, UNICODE_FALLBACK_FONT_PATHS, loaded_paths);
    }
}

fn load_requested_font_family(
    database: &mut usvg::fontdb::Database,
    family: &str,
    loaded_paths: &mut BTreeSet<std::path::PathBuf>,
) {
    if family == "sans-serif" {
        let configured = configured_html_sans_serif_family().to_ascii_lowercase();
        load_font_paths_once(database, paths_for_font_family(&configured), loaded_paths);
        load_font_paths_once(database, ARIAL_FONT_PATHS, loaded_paths);
        load_font_paths_once(database, LIBERATION_SANS_FONT_PATHS, loaded_paths);
    } else {
        load_font_paths_once(database, paths_for_font_family(family), loaded_paths);
    }
}

fn cjk_fallback_font_paths(request: &HtmlFontRequest) -> Vec<&'static str> {
    let has_serif = request.families.iter().any(|family| family == "serif");
    let has_sans_serif = request.families.iter().any(|family| family == "sans-serif");
    let paths = if has_serif && has_sans_serif {
        CJK_FONT_PATHS
            .iter()
            .chain(CJK_SERIF_FONT_PATHS)
            .copied()
            .collect()
    } else if has_serif {
        CJK_SERIF_FONT_PATHS.to_vec()
    } else {
        CJK_FONT_PATHS.to_vec()
    };
    if interface_bold_cjk_request(request) {
        /* WHY: CoreTextのsystem UIは通常CJK書体ではなく1083 UPMのinterface faceを使う。 */
        SYSTEM_CJK_FONT_PATHS.iter().copied().chain(paths).collect()
    } else {
        paths
    }
}

pub(super) fn is_generic_font_family(family: &str) -> bool {
    matches!(
        family,
        "serif"
            | "sans-serif"
            | "monospace"
            | "cursive"
            | "fantasy"
            | "system-ui"
            | "-apple-system"
            | "blinkmacsystemfont"
    )
}

fn paths_for_font_family(family: &str) -> &'static [&'static str] {
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

#[cfg(test)]
#[path = "svg_rasterize_html_font_sources_platform_tests.rs"]
mod platform_tests;
#[cfg(test)]
#[path = "svg_rasterize_html_font_sources_tests.rs"]
mod tests;
