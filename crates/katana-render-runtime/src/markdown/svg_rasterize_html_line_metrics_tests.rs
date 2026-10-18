use super::{
    clear_html_line_metrics_cache, html_line_metrics, html_line_metrics_cache_len,
    html_text_baseline, html_text_baseline_for_text,
};

#[test]
fn system_font_baseline_uses_the_primary_font_strut() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    for (size, height, baseline) in [
        (16.0, 24.0, 18.0),
        (17.0, 26.0, 19.0),
        (22.0, 33.0, 24.0),
        (30.0, 37.5, 30.0),
    ] {
        let actual = html_text_baseline(family, size, 700, false, height)
            .ok_or_else(|| "missing system font strut".to_string())?;
        assert_eq!(actual, baseline, "font size {size}");
    }
    Ok(())
}

#[test]
fn normal_and_explicit_baselines_use_their_css_strut_modes() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    let footer = html_text_baseline_for_text(
        "この画面は架空の図書館サービスを用いた描画検証用fixtureです。",
        family,
        13.0,
        400,
        false,
        20.0,
        true,
    )
    .ok_or_else(|| "missing footer baseline".to_string())?;
    assert_eq!(footer, 14.0);
    let reversed =
        html_text_baseline_for_text("fixture日本語", family, 13.0, 400, false, 20.0, true)
            .ok_or_else(|| "missing reversed baseline".to_string())?;
    assert_eq!(reversed, 14.0);
    let badge = html_text_baseline_for_text("要件仕様 v1", family, 12.0, 700, false, 18.0, true)
        .ok_or_else(|| "missing badge baseline".to_string())?;
    assert_eq!(badge, 14.0);
    let explicit =
        html_text_baseline_for_text("fixture日本語", family, 13.0, 400, false, 20.0, false)
            .ok_or_else(|| "missing explicit baseline".to_string())?;
    assert_eq!(explicit, 15.0);
    Ok(())
}

#[test]
fn line_metrics_cache_clear_discards_previous_entries() -> Result<(), String> {
    clear_html_line_metrics_cache();
    assert_eq!(html_line_metrics_cache_len(), 0);
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    html_line_metrics("日本語", family, 13.0, 400, false)
        .ok_or_else(|| "missing line metrics".to_string())?;
    assert_eq!(html_line_metrics_cache_len(), 1);
    clear_html_line_metrics_cache();
    assert_eq!(html_line_metrics_cache_len(), 0);
    html_line_metrics("日本語", family, 13.0, 400, false)
        .ok_or_else(|| "missing line metrics after clear".to_string())?;
    assert_eq!(html_line_metrics_cache_len(), 1);
    Ok(())
}

#[test]
fn japanese_system_font_normal_line_height_matches_chrome() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    for (size, height) in [(16.0, 24.0), (17.0, 26.0), (22.0, 33.0)] {
        let (ascent, descent) = html_line_metrics("日本語", family, size, 700, false)
            .ok_or_else(|| "missing installed Japanese font metrics".to_string())?;
        assert_eq!(ascent + descent, height, "font size {size}");
    }
    let (ascent, descent) = html_line_metrics("日本語", family, 16.0, 400, false)
        .ok_or_else(|| "missing normal Japanese font metrics".to_string())?;
    assert_eq!(ascent + descent, 24.0);
    Ok(())
}

#[test]
fn japanese_system_font_advance_matches_chrome_dom() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    let text = "検索結果は通信環境や蔵書の更新状況によって表示順が変わる場合があり";
    let width = super::super::shaped_text_width(text, family, 16.0, 400, false, 0.0, None)
        .ok_or_else(|| "missing shaped font advance".to_string())?;
    assert!((width - 527.531_25).abs() <= 1.0 / 64.0, "width {width}");
    Ok(())
}

#[test]
fn variable_system_font_axes_match_chrome_dom() -> Result<(), String> {
    let family = "-apple-system, BlinkMacSystemFont, Hiragino Sans, sans-serif";
    for (text, weight, expected) in [
        ("AV", 400, 26.625),
        ("AV", 700, 28.828_125),
        ("目的", 700, 44.0),
        ("1. ", 700, 20.406_25),
        ("1. 目的", 700, 64.406_25),
    ] {
        let actual = super::super::shaped_text_width(text, family, 22.0, weight, false, 0.0, None)
            .ok_or_else(|| "missing shaped system font".to_string())?;
        assert!(
            (actual - expected).abs() <= 1.0 / 64.0,
            "{text} {weight}: {actual}"
        );
    }
    Ok(())
}
