use super::text_shaping::{
    FontShape, TextSpacingTrim, shaped_text_width_in_database_with_trim,
    shaped_text_width_with_trim,
};
use super::{HtmlTextMeasurementToken, SvgRasterizeError};
use resvg::usvg;
use std::sync::Arc;

#[path = "svg_rasterize_text_width_fallback.rs"]
mod width_fallback;
use width_fallback::fallback_text_width;
#[cfg(test)]
use width_fallback::{canonical_svg_font_family, escape_xml_attribute, escape_xml_text};

pub(super) struct HtmlTextMeasure<'a> {
    font_family: &'a str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&'a str>,
    trim: TextSpacingTrim,
}

impl<'a> HtmlTextMeasure<'a> {
    pub(super) fn new(
        font_family: &'a str,
        font_size: f32,
        font_style: (u16, bool),
        letter_spacing: f32,
        font_feature_settings: Option<&'a str>,
    ) -> Self {
        Self {
            font_family,
            font_size,
            font_weight: font_style.0,
            italic: font_style.1,
            letter_spacing,
            font_feature_settings,
            trim: TextSpacingTrim::Normal,
        }
    }

    pub(super) fn with_trim(mut self, trim: TextSpacingTrim) -> Self {
        self.trim = trim;
        self
    }
}

#[path = "svg_rasterize_text_prepared.rs"]
mod prepared;
pub(super) use prepared::PreparedHtmlTextMeasurement;

pub(super) fn measure_html_text(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Result<f32, SvgRasterizeError> {
    measure_html_text_with_trim(
        text,
        font_family,
        font_size,
        (font_weight, italic),
        letter_spacing,
        font_feature_settings,
        TextSpacingTrim::Normal,
    )
}

pub(super) fn measure_html_text_with_trim(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_style: (u16, bool),
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
    trim: TextSpacingTrim,
) -> Result<f32, SvgRasterizeError> {
    HtmlTextMeasure::new(
        font_family,
        font_size,
        font_style,
        letter_spacing,
        font_feature_settings,
    )
    .with_trim(trim)
    .measure(text)
}

#[cfg(test)]
pub(super) fn measure_html_text_with_cache_status(
    text: &str,
    font_family: &str,
    font_size: f32,
    font_weight: u16,
    italic: bool,
    letter_spacing: f32,
    font_feature_settings: Option<&str>,
) -> Result<(f32, bool), SvgRasterizeError> {
    HtmlTextMeasure {
        font_family,
        font_size,
        font_weight,
        italic,
        letter_spacing,
        font_feature_settings,
        trim: TextSpacingTrim::Normal,
    }
    .measure_with_status(text)
}

impl HtmlTextMeasure<'_> {
    fn measure(&self, text: &str) -> Result<f32, SvgRasterizeError> {
        self.measure_with_status(text).map(|(width, _)| width)
    }

    fn measure_with_status(&self, text: &str) -> Result<(f32, bool), SvgRasterizeError> {
        self.measure_with_status_in_database(text, None)
    }

    fn measure_with_status_in_database(
        &self,
        text: &str,
        database: Option<&Arc<usvg::fontdb::Database>>,
    ) -> Result<(f32, bool), SvgRasterizeError> {
        if text.is_empty() {
            return Ok((0.0, false));
        }
        let shaped = database.map_or_else(
            || self.shaped_measure(text),
            |database| {
                shaped_text_width_in_database_with_trim(
                    database,
                    text,
                    self.font_family,
                    FontShape::new(self.font_size, self.font_weight, self.italic)
                        .with_trim(self.trim),
                    self.letter_spacing,
                    self.font_feature_settings,
                )
            },
        );
        shaped.map_or_else(
            || self.fallback_measure(text).map(|width| (width, false)),
            |width| Ok((width, true)),
        )
    }

    fn shaped_measure(&self, text: &str) -> Option<f32> {
        shaped_text_width_with_trim(
            text,
            self.font_family,
            FontShape::new(self.font_size, self.font_weight, self.italic).with_trim(self.trim),
            self.letter_spacing,
            self.font_feature_settings,
        )
    }

    fn fallback_measure(&self, text: &str) -> Result<f32, SvgRasterizeError> {
        fallback_text_width(
            text,
            self.font_family,
            self.font_size,
            self.font_weight,
            self.italic,
            self.letter_spacing,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{TextSpacingTrim, fallback_text_width, measure_html_text};

    const PREPARED_CASES: [(&str, &str, u16, bool); 3] = [
        ("Prepared AV", "Arial", 400, false),
        ("Prepared AV", "Arial", 700, true),
        ("日本 AV", "Noto Sans", 400, false),
    ];

    #[test]
    fn prepared_token_and_width_keep_one_database_snapshot_and_legacy_status()
    -> Result<(), Box<dyn std::error::Error>> {
        super::super::SvgRasterizeOps::with_html_font_render_scope(|| {
            for (text, family, weight, italic) in PREPARED_CASES {
                let prepared = super::super::SvgRasterizeOps::prepare_html_text_measurement(
                    text,
                    family,
                    16.0,
                    weight,
                    italic,
                    1.0,
                    Some("'kern' 1"),
                );
                let actual = measure_prepared_snapshot(&prepared)?;
                let expected = super::measure_html_text_with_cache_status(
                    text,
                    family,
                    16.0,
                    weight,
                    italic,
                    1.0,
                    Some("'kern' 1"),
                )?;
                assert_eq!(actual, expected);
            }
            Ok(())
        })
    }

    fn measure_prepared_snapshot(
        prepared: &super::PreparedHtmlTextMeasurement<'_>,
    ) -> Result<(f32, bool), super::SvgRasterizeError> {
        let snapshot = std::sync::Arc::clone(prepared.database_snapshot());
        let token_key = prepared.token().map(super::HtmlTextMeasurementToken::key);
        let actual = prepared.measure_with_cache_status()?;
        assert!(std::sync::Arc::ptr_eq(
            &snapshot,
            prepared.database_snapshot()
        ));
        assert_eq!(
            token_key,
            prepared.token().map(super::HtmlTextMeasurementToken::key)
        );
        Ok(actual)
    }

    #[cfg(unix)]
    #[test]
    fn selected_database_memo_rejects_same_key_different_arcs()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = PreparedGenerationFixture::new()?;
        let replacement = load_selected_binding_database(&fixture.path);
        let matching = std::sync::Arc::clone(&fixture.database);
        assert!(!std::sync::Arc::ptr_eq(&matching, &replacement));
        super::super::SvgRasterizeOps::with_html_font_render_scope(|| {
            let selected = |database| {
                super::HtmlTextMeasurementToken::for_selected_database(
                    "Prepared Generation Fixture",
                    &fixture.key,
                    database,
                )
            };
            let first = selected(&fixture.database).ok_or("real font token missing")?;
            assert_eq!(
                selected(&matching).map(|token| token.key()),
                Some(first.key())
            );
            assert!(selected(&replacement).is_none());
            assert_selected_binding_width_matches_legacy(&replacement)?;
            Ok(())
        })
    }

    #[cfg(unix)]
    #[test]
    fn selected_database_memo_rejects_expired_proof_database()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = PreparedGenerationFixture::new()?;
        super::super::SvgRasterizeOps::with_html_font_render_scope(|| {
            let database = load_selected_binding_database(&fixture.path);
            let selected = |database: &std::sync::Arc<resvg::usvg::fontdb::Database>| {
                super::HtmlTextMeasurementToken::for_selected_database(
                    "Prepared Generation Fixture",
                    &fixture.key,
                    database,
                )
            };
            let token = selected(&database).ok_or("real font token missing")?;
            drop(database);
            assert!(!token.is_usable());
            assert!(selected(&fixture.database).is_none());
            Ok(())
        })
    }

    #[cfg(unix)]
    fn load_selected_binding_database(
        path: &std::path::Path,
    ) -> std::sync::Arc<resvg::usvg::fontdb::Database> {
        let mut database = resvg::usvg::fontdb::Database::new();
        assert!(
            !database
                .load_font_source(resvg::usvg::fontdb::Source::File(path.to_path_buf()))
                .is_empty()
        );
        std::sync::Arc::new(database)
    }

    #[cfg(unix)]
    fn assert_selected_binding_width_matches_legacy(
        database: &std::sync::Arc<resvg::usvg::fontdb::Database>,
    ) -> Result<(), super::SvgRasterizeError> {
        let style = super::HtmlTextMeasure {
            font_family: "Noto Sans",
            font_size: 16.0,
            font_weight: 400,
            italic: false,
            letter_spacing: 0.0,
            font_feature_settings: None,
            trim: TextSpacingTrim::Normal,
        };
        let actual = style.measure_with_status_in_database("A", Some(database))?;
        let expected = super::measure_html_text_with_cache_status(
            "A",
            "Noto Sans",
            16.0,
            400,
            false,
            0.0,
            None,
        )?;
        assert!(actual.1);
        assert_eq!(actual, expected);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn prepared_requests_restart_after_real_file_generation_changes()
    -> Result<(), Box<dyn std::error::Error>> {
        let fixture = PreparedGenerationFixture::new()?;
        let mut tokens = Vec::new();
        let mut rewrite = Ok::<(), std::io::Error>(());
        super::super::SvgRasterizeOps::with_html_font_render_scope(|| {
            rewrite = record_prepared_generation_attempt(&fixture, &mut tokens);
        });
        rewrite?;
        assert_eq!(tokens.len(), 2);
        assert!(tokens[0].is_some());
        assert!(tokens[1].is_none());
        assert!(super::super::SvgRasterizeOps::with_html_font_render_scope(
            || {
                super::super::SvgRasterizeOps::prepare_html_text_measurement(
                    "A",
                    "Noto Sans",
                    16.0,
                    400,
                    false,
                    0.0,
                    None,
                )
                .token()
                .is_some()
            }
        ));
        Ok(())
    }

    #[cfg(unix)]
    const PREPARED_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");

    #[cfg(unix)]
    struct PreparedGenerationFixture {
        _directory: super::probe_test_support::TempProbeDirectory,
        path: std::path::PathBuf,
        key: super::super::font::DatabaseKey,
        database: std::sync::Arc<resvg::usvg::fontdb::Database>,
    }

    #[cfg(unix)]
    impl PreparedGenerationFixture {
        fn new() -> Result<Self, Box<dyn std::error::Error>> {
            use resvg::usvg::fontdb::{Database, Source};
            let directory = super::probe_test_support::TempProbeDirectory::create()?;
            let path = directory.file("prepared-generation.ttf");
            std::fs::write(&path, PREPARED_FONT_BYTES)?;
            let mut database = Database::new();
            assert!(
                !database
                    .load_font_source(Source::File(path.clone()))
                    .is_empty()
            );
            let key = super::super::font::DatabaseKey::eager(
                super::super::font::HtmlFontRequest::from_text("Prepared Generation Fixture", "A"),
            );
            Ok(Self {
                _directory: directory,
                path,
                key,
                database: std::sync::Arc::new(database),
            })
        }
    }

    #[cfg(unix)]
    fn record_prepared_generation_attempt(
        fixture: &PreparedGenerationFixture,
        tokens: &mut Vec<Option<super::HtmlTextMeasurementToken>>,
    ) -> std::io::Result<()> {
        let prepared = super::super::SvgRasterizeOps::prepare_html_text_measurement(
            "A",
            "Noto Sans",
            16.0,
            400,
            false,
            0.0,
            None,
        );
        tokens.push(prepared.token().cloned());
        let fixture_token = super::HtmlTextMeasurementToken::for_selected_database(
            "Prepared Generation Fixture",
            &fixture.key,
            &fixture.database,
        );
        if tokens.len() == 1 {
            assert!(fixture_token.is_some());
            let mut changed = PREPARED_FONT_BYTES.to_vec();
            changed.push(0);
            std::fs::write(&fixture.path, changed)?;
        }
        Ok(())
    }

    #[cfg(any(target_os = "macos", target_os = "ios"))]
    #[test]
    fn macos_system_aliases_are_canonicalized_for_svg_paint() {
        assert_eq!(
            super::canonical_svg_font_family("-apple-system, sans-serif"),
            "System Font, sans-serif"
        );
    }

    #[test]
    fn empty_html_text_has_zero_width() {
        let measured = measure_html_text("", "sans-serif", 16.0, 400, false, 0.0, None);

        assert!(matches!(measured, Ok(0.0)));
    }

    #[test]
    fn fallback_measurement_supports_bold_italic_text() {
        let measured = fallback_text_width("Bold", "sans-serif", 16.0, 700, true, 0.0);

        assert!(matches!(measured, Ok(width) if width > 0.0));
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_bbox_probe.rs"]
mod bbox_probe;

#[cfg(test)]
#[path = "svg_rasterize_text_html_png_probe.rs"]
mod html_png_probe;

#[cfg(test)]
#[path = "svg_rasterize_text_probe_test_support.rs"]
pub(super) mod probe_test_support;

#[cfg(test)]
#[path = "svg_rasterize_text_probe_file_contract_tests.rs"]
mod probe_file_contract_tests;
