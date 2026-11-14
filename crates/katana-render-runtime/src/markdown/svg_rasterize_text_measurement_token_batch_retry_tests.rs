use super::super::font_file_fixture::{self as shared_font_file_fixture, FontFileFixture};
use super::HtmlTextMeasurementToken;
use resvg::usvg::fontdb::Source;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

type TestResult<T> = Result<T, Box<dyn std::error::Error>>;

fn font_file_fixture() -> TestResult<FontFileFixture> {
    let path = std::env::temp_dir().join(format!(
        "krr-token-batch-retry-{}-{}.ttf",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    shared_font_file_fixture::create(path, FONT_BYTES)
}

#[cfg(unix)]
#[test]
fn changed_file_generation_retries_real_token_factory_without_reusing_batch_memo() -> TestResult<()>
{
    let fixture = font_file_fixture()?;
    let mut token_results = Vec::new();
    let mut fixture_rewrite = Ok::<(), std::io::Error>(());
    super::super::with_render_scoped_stamp_batch(|| {
        let token = HtmlTextMeasurementToken::for_request("A", "Noto Sans");
        token_results.push(token);
        let generation =
            super::super::font::font_source_generation(&fixture.database, fixture.face_id);
        assert!(generation.is_file());
        assert!(generation.durable_reusable());
        if token_results.len() == 1 {
            let mut changed = FONT_BYTES.to_vec();
            changed.push(0);
            fixture_rewrite = std::fs::write(&fixture.path, changed);
        }
    });

    fixture_rewrite?;
    assert_eq!(token_results.len(), 2);
    assert!(token_results[0].is_some());
    assert!(token_results[1].is_none());
    assert!(super::super::with_render_scoped_stamp_batch(|| {
        HtmlTextMeasurementToken::for_request("A", "Noto Sans").is_some()
    }));
    Ok(())
}

#[cfg(windows)]
#[test]
fn windows_file_generation_keeps_nondurable_token_policy() -> TestResult<()> {
    let fixture = font_file_fixture()?;
    super::super::with_render_scoped_stamp_batch(|| {
        let generation =
            super::super::font::font_source_generation(&fixture.database, fixture.face_id);
        assert!(generation.is_file());
        assert!(!generation.durable_reusable());
        assert!(HtmlTextMeasurementToken::for_request("A", "Noto Sans").is_none());
    });
    Ok(())
}

#[cfg(target_os = "macos")]
mod overflow_tests {
    use super::{FontFileFixture, HtmlTextMeasurementToken, PathBuf, Source, TestResult};
    use crate::markdown::svg_rasterize::text_shaping::font;

    const STAMP_CAPACITY: usize = 256;
    const FAMILY: &str = "Helvetica Neue";

    struct FontAliases {
        paths: Vec<PathBuf>,
        fixture: FontFileFixture,
        faces: Vec<resvg::usvg::fontdb::ID>,
    }

    impl Drop for FontAliases {
        fn drop(&mut self) {
            for path in &self.paths {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    fn real_aliases() -> TestResult<FontAliases> {
        let mut aliases = FontAliases {
            paths: Vec::new(),
            fixture: super::font_file_fixture()?,
            faces: Vec::new(),
        };
        for index in 0..STAMP_CAPACITY {
            let path = aliases
                .fixture
                .path
                .with_extension(format!("alias-{index}.ttf"));
            std::fs::hard_link(&aliases.fixture.path, &path)?;
            aliases.paths.push(path.clone());
            aliases
                .fixture
                .database
                .load_font_source(Source::File(path));
            aliases.faces.push(
                aliases
                    .fixture
                    .database
                    .faces()
                    .last()
                    .ok_or("alias font missing")?
                    .id,
            );
        }
        Ok(aliases)
    }

    fn fill_stamp_capacity(aliases: &FontAliases) {
        for face in &aliases.faces {
            let generation = font::font_source_generation(&aliases.fixture.database, *face);
            assert!(generation.reusable());
            assert!(generation.durable_reusable());
        }
        assert!(font::memo_usable());
    }

    #[test]
    fn real_stamp_overflow_records_a_miss_without_changing_system_fonts() -> TestResult<()> {
        let aliases = real_aliases()?;
        let database = super::super::super::html_font_db_for_text(FAMILY, "A");
        assert!(
            database
                .faces()
                .any(|face| matches!(face.source, Source::File(_)))
        );
        let mut attempts = 0;
        font::with_validated_stamp_batch(|| {
            attempts += 1;
            let _token_scope = super::super::Scope::enter();
            if !font::memo_usable() {
                assert!(HtmlTextMeasurementToken::for_request("A", FAMILY).is_none());
                return;
            }
            fill_stamp_capacity(&aliases);
            assert!(HtmlTextMeasurementToken::for_request("A", FAMILY).is_none());
            super::super::ACTIVE_RENDER_MEMO.with(|memo| {
                assert!(memo.borrow().as_ref().is_some_and(|entries| {
                    matches!(entries.as_slice(), [super::super::MemoEntry::Miss(_)])
                }));
            });
        });
        assert_eq!(attempts, 2);
        Ok(())
    }
}
