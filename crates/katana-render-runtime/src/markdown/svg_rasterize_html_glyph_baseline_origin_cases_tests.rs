pub(super) struct BaselineOriginCase {
    pub(super) id: &'static str,
    pub(super) expected_y: f32,
    pub(super) glyph_count: usize,
}

const FALLBACK_BASELINE_CASE_COUNT: usize = 12;

pub(super) const MIXED_BASELINE_ORIGINS: [BaselineOriginCase; FALLBACK_BASELINE_CASE_COUNT] = [
    BaselineOriginCase {
        id: "Trebuchet-MS-central-12-mixed",
        expected_y: 44.0,
        glyph_count: 4,
    },
    BaselineOriginCase {
        id: "Trebuchet-MS-central-12-fallback-only",
        expected_y: 84.0,
        glyph_count: 2,
    },
    BaselineOriginCase {
        id: "Helvetica-mathematical-12-mixed",
        expected_y: 126.0,
        glyph_count: 4,
    },
    BaselineOriginCase {
        id: "Helvetica-mathematical-12-fallback-only",
        expected_y: 166.0,
        glyph_count: 2,
    },
    BaselineOriginCase {
        id: "Trebuchet-MS-central-14-mixed",
        expected_y: 205.0,
        glyph_count: 4,
    },
    BaselineOriginCase {
        id: "Trebuchet-MS-central-14-fallback-only",
        expected_y: 245.0,
        glyph_count: 2,
    },
    BaselineOriginCase {
        id: "Helvetica-mathematical-14-mixed",
        expected_y: 287.0,
        glyph_count: 4,
    },
    BaselineOriginCase {
        id: "Helvetica-mathematical-14-fallback-only",
        expected_y: 327.0,
        glyph_count: 2,
    },
    BaselineOriginCase {
        id: "Trebuchet-MS-central-16-mixed",
        expected_y: 366.0,
        glyph_count: 4,
    },
    BaselineOriginCase {
        id: "Trebuchet-MS-central-16-fallback-only",
        expected_y: 406.0,
        glyph_count: 2,
    },
    BaselineOriginCase {
        id: "Helvetica-mathematical-16-mixed",
        expected_y: 447.0,
        glyph_count: 4,
    },
    BaselineOriginCase {
        id: "Helvetica-mathematical-16-fallback-only",
        expected_y: 487.0,
        glyph_count: 2,
    },
];
