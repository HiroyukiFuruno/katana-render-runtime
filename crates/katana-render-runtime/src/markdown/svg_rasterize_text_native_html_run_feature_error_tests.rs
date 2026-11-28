use super::super::test_support::regular_shape;
use super::{
    SelectedFace, TextDxState, append_native_metrics,
    shape_run_character_metrics_without_features_with_variations,
};
use crate::markdown::svg_rasterize::{SvgRasterizeOps, font_file_fixture::FONT_BYTES};
use resvg::usvg::fontdb::{Database, Source};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

const HAN_PUNCTUATION: &str = "A、";
const SFNT_SIGNATURE_BYTES: usize = 4;

struct FontSource {
    invalid_header: Vec<u8>,
    invalid: AtomicBool,
}

impl AsRef<[u8]> for FontSource {
    fn as_ref(&self) -> &[u8] {
        if self.invalid.load(Ordering::Relaxed) {
            &self.invalid_header
        } else {
            FONT_BYTES
        }
    }
}

#[test]
fn native_metrics_reject_feature_parsing_after_the_font_header_changes() -> Result<(), String> {
    let mut invalid_header = FONT_BYTES.to_vec();
    invalid_header[..SFNT_SIGNATURE_BYTES].fill(0);
    let source = Arc::new(FontSource {
        invalid_header,
        invalid: AtomicBool::new(false),
    });
    let face = face_from_binary_source(source.clone())?;
    SvgRasterizeOps::with_html_font_render_scope(|| {
        assert!(has_natural_metrics(&face));
        source.invalid.store(true, Ordering::Relaxed);
        assert!(skrifa::FontRef::from_index(source.as_ref().as_ref(), 0).is_err());
        assert!(has_natural_metrics(&face));
        let mut state = TextDxState::new(HAN_PUNCTUATION.chars().count());
        assert_eq!(
            append_native_metrics(&mut state, &face, HAN_PUNCTUATION, regular_shape(), None),
            None
        );
    });
    Ok(())
}

fn has_natural_metrics(face: &SelectedFace) -> bool {
    shape_run_character_metrics_without_features_with_variations(
        &face.database,
        face.id,
        HAN_PUNCTUATION,
        regular_shape(),
        &face.variations,
    )
    .is_some()
}

fn face_from_binary_source(source: Arc<FontSource>) -> Result<SelectedFace, String> {
    let mut database = Database::new();
    database.load_font_source(Source::Binary(source));
    let id = database
        .faces()
        .next()
        .ok_or("font fixture did not load")?
        .id;
    Ok(SelectedFace {
        database: Arc::new(database),
        id,
        alias: "Noto Sans".into(),
        variations: Vec::new(),
    })
}
