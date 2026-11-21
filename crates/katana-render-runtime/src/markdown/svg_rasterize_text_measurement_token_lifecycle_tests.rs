use super::HtmlTextMeasurementToken;
#[cfg(unix)]
use std::collections::HashSet;

const FAMILY: &str = "Noto Sans, serif";
const FAMILIES: [&str; 9] = [
    "Noto Sans, serif, missing-0",
    "Noto Sans, serif, missing-1",
    "Noto Sans, serif, missing-2",
    "Noto Sans, serif, missing-3",
    "Noto Sans, serif, missing-4",
    "Noto Sans, serif, missing-5",
    "Noto Sans, serif, missing-6",
    "Noto Sans, serif, missing-7",
    "Noto Sans, serif, missing-8",
];
const TEXTS: [&str; 9] = ["A", "B", "C", "D", "E", "F", "G", "H", "I"];

fn scoped_token(text: &str, family: &str) -> Option<HtmlTextMeasurementToken> {
    super::super::with_render_scoped_stamp_batch(|| {
        HtmlTextMeasurementToken::for_request(text, family)
    })
}

#[cfg(unix)]
#[test]
fn nine_sequential_scopes_keep_real_font_requests_bounded() {
    let tokens = TEXTS
        .iter()
        .zip(FAMILIES)
        .map(|(text, family)| scoped_token(text, family))
        .collect::<Vec<_>>();
    assert!(tokens.iter().all(Option::is_some));
    let first_key = tokens[0].as_ref().map(HtmlTextMeasurementToken::key);
    let replacement = scoped_token(TEXTS[0], FAMILIES[0]);
    assert!(replacement.is_some());
    assert_ne!(
        first_key,
        replacement.as_ref().map(HtmlTextMeasurementToken::key)
    );
}

#[cfg(windows)]
#[test]
fn windows_nondurable_requests_are_rejected() {
    for (text, family) in TEXTS.iter().zip(FAMILIES) {
        assert!(scoped_token(text, family).is_none());
    }
}

#[cfg(unix)]
#[test]
fn ninth_request_is_rejected_by_active_scope_cap() {
    super::super::with_render_scoped_stamp_batch(|| {
        for (text, family) in TEXTS[..8].iter().zip(FAMILIES) {
            assert!(HtmlTextMeasurementToken::for_request(text, family).is_some());
        }
        assert!(HtmlTextMeasurementToken::for_request(TEXTS[8], FAMILIES[8]).is_none());
        let weight = resvg::usvg::fontdb::Weight::NORMAL.0;
        let token =
            HtmlTextMeasurementToken::for_measured_text(TEXTS[8], FAMILIES[8], weight, false);
        assert!(token.is_none());
    });
}

#[cfg(windows)]
#[test]
fn windows_nested_scope_preserves_nondurable_policy() {
    super::super::with_render_scoped_stamp_batch(|| {
        assert!(HtmlTextMeasurementToken::for_request("A", FAMILY).is_none());
    });
}

#[cfg(unix)]
#[test]
fn key_debug_and_hash_follow_token_identity() {
    super::super::with_render_scoped_stamp_batch(|| {
        let token = HtmlTextMeasurementToken::for_request("A", FAMILY);
        assert!(token.is_some());
        if let Some(token) = token {
            let mut keys = HashSet::new();
            keys.insert(token.key());
            assert!(keys.contains(&token.clone().key()));
            assert!(format!("{:?}", token.key()).contains("HtmlTextMeasurementTokenKey"));
        }
    });
}

#[cfg(unix)]
#[test]
fn nested_scope_and_unwind_restore_token_state() {
    super::super::with_render_scoped_stamp_batch(|| {
        let outer = HtmlTextMeasurementToken::for_request("A", FAMILY);
        assert!(outer.is_some());
        let nested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            super::super::with_render_scoped_stamp_batch(|| {
                assert!(HtmlTextMeasurementToken::for_request("B", FAMILY).is_none());
                std::panic::resume_unwind(Box::new(()));
            });
        }));
        assert!(nested.is_err());
        let restored = HtmlTextMeasurementToken::for_request("A", FAMILY);
        assert!(restored.is_some());
        if let (Some(outer), Some(restored)) = (outer, restored) {
            assert_eq!(outer.key(), restored.key());
        }
    });
}

#[cfg(unix)]
#[test]
fn top_level_unwind_releases_token_scope() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        super::super::with_render_scoped_stamp_batch(|| {
            assert!(HtmlTextMeasurementToken::for_request("A", FAMILY).is_some());
            std::panic::resume_unwind(Box::new(()));
        });
    }));
    assert!(result.is_err());
    assert!(scoped_token("A", FAMILY).is_some());
}
