use super::{REGULAR_WEIGHT, text, variable_tree};
use resvg::usvg::{self, Tree, fontdb::ID};
use skrifa::raw::TableProvider;
use svg_harfrust::{
    FontRef, ShapeOptions, ShaperData, ShaperInstance, Tag, UnicodeBuffer, Variation,
};

const SIZES: &[f32] = &[10.0, 11.0, 12.0, 12.6];
const DEFAULT_POINT_SIZE: f32 = 12.0;
const OPTICAL_SIZE: f32 = 17.0;
const SAMPLE: &str = "HHHH";
const POSITION_EPSILON: f32 = 0.0001;

#[derive(Debug)]
struct TrackingCase {
    size: f32,
    actual: Vec<f32>,
    explicit: Vec<f32>,
    default: Vec<f32>,
    twelve: Vec<f32>,
}

struct RawRun {
    ids: Vec<u32>,
    advances: Vec<i32>,
}

struct RawComparison {
    explicit: RawRun,
    default: RawRun,
    twelve: RawRun,
    upem: u16,
}

#[test]
fn sfns_svg_tracking_matches_explicit_css_point_size() -> Result<(), String> {
    let cases = tracking_cases()?;
    eprintln!("SFNS point-size tracking cases: {cases:?}");
    assert!(
        cases
            .iter()
            .all(|case| matching_advances(&case.actual, &case.explicit)),
        "SVG layout differs from explicit CSS point-size oracle: {cases:?}"
    );
    Ok(())
}

#[test]
fn sfns_tracking_default_is_twelve_css_pixels() -> Result<(), String> {
    let cases = tracking_cases()?;
    for case in &cases {
        assert_eq!(case.default, case.twelve);
        if case.size == DEFAULT_POINT_SIZE {
            assert_eq!(case.default, case.explicit);
        } else {
            assert!(
                !matching_advances(&case.default, &case.explicit),
                "SFNS fixture must expose size-dependent tracking: {case:?}"
            );
        }
    }
    Ok(())
}

fn tracking_cases() -> Result<Vec<TrackingCase>, String> {
    SIZES.iter().copied().map(tracking_case).collect()
}

fn tracking_case(size: f32) -> Result<TrackingCase, String> {
    let tree = tracking_tree(size)?;
    let span = text(&tree, "tracking")?
        .layouted()
        .first()
        .ok_or("tracking span missing")?;
    let glyphs = &span.positioned_glyphs;
    let face = glyphs.first().ok_or("tracking glyph missing")?.font;
    assert!(glyphs.iter().all(|glyph| glyph.font == face));
    let raw = comparison_for_face(&tree, face, size)?;
    assert_glyph_ids(glyphs, &raw);
    Ok(TrackingCase {
        size,
        actual: observed_advances(glyphs),
        explicit: svg_advances(&raw.explicit, size, raw.upem),
        default: svg_advances(&raw.default, size, raw.upem),
        twelve: svg_advances(&raw.twelve, size, raw.upem),
    })
}

fn tracking_tree(size: f32) -> Result<Tree, String> {
    let markup = format!(
        r#"<text id="tracking" x="10" y="30" font-size="{size}" font-weight="{REGULAR_WEIGHT}" font-optical-sizing="none" font-variation-settings="'opsz' {OPTICAL_SIZE}">{SAMPLE}</text>"#,
    );
    variable_tree(&markup)
}

fn observed_advances(glyphs: &[usvg::layout::PositionedGlyph]) -> Vec<f32> {
    glyphs
        .windows(2)
        .map(|pair| pair[1].transform().tx - pair[0].transform().tx)
        .collect()
}

fn comparison_for_face(tree: &Tree, face: ID, size: f32) -> Result<RawComparison, String> {
    tree.fontdb()
        .with_face_data(face, |data, index| {
            let font =
                skrifa::FontRef::from_index(data, index).map_err(|error| error.to_string())?;
            let upem = font
                .head()
                .map_err(|error| error.to_string())?
                .units_per_em();
            Ok(RawComparison {
                explicit: raw_run(data, index, Some(size))?,
                default: raw_run(data, index, None)?,
                twelve: raw_run(data, index, Some(DEFAULT_POINT_SIZE))?,
                upem,
            })
        })
        .ok_or("tracking font bytes missing")?
}

fn raw_run(data: &[u8], index: u32, point_size: Option<f32>) -> Result<RawRun, String> {
    let font = FontRef::from_index(data, index).map_err(|error| error.to_string())?;
    let variations = tracking_variations();
    let instance = ShaperInstance::from_variations(&font, &variations);
    let shaper_data = ShaperData::new(&font);
    let shaper = shaper_data.shaper(&font).instance(Some(&instance)).build();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(SAMPLE);
    buffer.guess_segment_properties();
    let output = shaper.shape(buffer, ShapeOptions::new().point_size(point_size));
    Ok(RawRun {
        ids: output
            .glyph_infos()
            .iter()
            .map(|info| info.glyph_id)
            .collect(),
        advances: output
            .glyph_positions()
            .iter()
            .map(|position| position.x_advance)
            .collect(),
    })
}

fn tracking_variations() -> Vec<Variation> {
    vec![
        Variation {
            tag: Tag::new(b"wght"),
            value: REGULAR_WEIGHT,
        },
        Variation {
            tag: Tag::new(b"opsz"),
            value: OPTICAL_SIZE,
        },
    ]
}

fn assert_glyph_ids(glyphs: &[usvg::layout::PositionedGlyph], raw: &RawComparison) {
    let ids = glyphs.iter().map(|glyph| glyph.id.0).collect::<Vec<_>>();
    assert_eq!(ids.len(), SAMPLE.chars().count());
    assert_eq!(ids, raw.explicit.ids);
    assert_eq!(ids, raw.default.ids);
    assert_eq!(ids, raw.twelve.ids);
}

fn svg_advances(run: &RawRun, size: f32, upem: u16) -> Vec<f32> {
    let scale = size / f32::from(upem);
    run.advances
        .iter()
        .take(SAMPLE.chars().count().saturating_sub(1))
        .map(|advance| *advance as f32 * scale)
        .collect()
}

fn matching_advances(actual: &[f32], expected: &[f32]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| (actual - expected).abs() <= POSITION_EPSILON)
}
