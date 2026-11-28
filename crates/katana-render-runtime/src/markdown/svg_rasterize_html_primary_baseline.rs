use super::{html_font_db_for_measured_text, matching_font_face};
use std::{cell::RefCell, collections::HashMap};

const MAX_PRIMARY_BASELINE_ENTRIES: usize = 256;

#[derive(Eq, Hash, PartialEq)]
struct BaselineKey {
    family: String,
    size: u32,
    weight: u16,
    italic: bool,
    line_height: u32,
}

thread_local! {
    static PRIMARY_BASELINE_CACHE: RefCell<HashMap<BaselineKey, f32>> = RefCell::new(HashMap::new());
}

pub(super) fn clear_primary_baseline_cache() {
    PRIMARY_BASELINE_CACHE.with(|cache| *cache.borrow_mut() = HashMap::new());
}

pub(super) fn primary_baseline(
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
    line_height: f32,
) -> Option<f32> {
    let key = BaselineKey {
        family: family.to_string(),
        size: size.to_bits(),
        weight,
        italic,
        line_height: line_height.to_bits(),
    };
    if let Some(baseline) = PRIMARY_BASELINE_CACHE.with(|cache| cache.borrow().get(&key).copied()) {
        super::super::super::measurement_diagnostic::baseline_finish(true, None);
        return Some(baseline);
    }
    let baseline = measure_primary_baseline(family, size, weight, italic, line_height)?;
    PRIMARY_BASELINE_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() >= MAX_PRIMARY_BASELINE_ENTRIES {
            cache.clear();
        }
        cache.insert(key, baseline);
    });
    Some(baseline)
}

/* WHY: 明示 line-height の primary strut は本文の glyph fallback に依存しない。 */
fn measure_primary_baseline(
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
    line_height: f32,
) -> Option<f32> {
    let started = super::super::super::measurement_diagnostic::baseline_start();
    let baseline = measure_primary_baseline_body(family, size, weight, italic, line_height);
    super::super::super::measurement_diagnostic::baseline_finish(false, started);
    baseline
}

fn measure_primary_baseline_body(
    family: &str,
    size: f32,
    weight: u16,
    italic: bool,
    line_height: f32,
) -> Option<f32> {
    let database = html_font_db_for_measured_text(family, "x0", weight, italic);
    let primary = matching_font_face(&database, family, weight, italic)?;
    measure_primary_baseline_for_face(&database, primary, size, line_height)
}

fn measure_primary_baseline_for_face(
    database: &resvg::usvg::fontdb::Database,
    primary: resvg::usvg::fontdb::ID,
    size: f32,
    line_height: f32,
) -> Option<f32> {
    database.with_face_data(primary, |data, index| {
        let face = rustybuzz::ttf_parser::Face::parse(data, index).ok()?;
        let scale = size / face.units_per_em() as f32;
        let ascent = (face.ascender() as f32 * scale).round();
        let descent = (-face.descender() as f32 * scale).round();
        Some(ascent + ((line_height - ascent - descent) / 2.0).floor())
    })?
}

#[cfg(test)]
#[path = "svg_rasterize_html_primary_baseline_tests.rs"]
mod tests;
