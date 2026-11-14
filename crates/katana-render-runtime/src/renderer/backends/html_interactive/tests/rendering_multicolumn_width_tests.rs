use super::{TestResult, element_box_for_id, start_with_viewport};

#[test]
fn column_width_without_count_fits_two_columns_and_gap() -> TestResult {
    let html = r#"<style>
        body { margin: 0; }
        main { width: 210px; column-width: 100px; column-gap: 10px; }
        main div { height: 10px; break-inside: avoid; }
    </style>
    <main><div id=first></div><div id=second></div>
          <div id=third></div><div id=fourth></div></main>"#;
    let mut session = start_with_viewport(html, 320, 240)?;
    let first = element_box_for_id(&mut session, "first")?;
    let second = element_box_for_id(&mut session, "second")?;
    let third = element_box_for_id(&mut session, "third")?;
    let fourth = element_box_for_id(&mut session, "fourth")?;
    let positions = [&first, &second, &third, &fourth]
        .map(|element| (element.x - first.x, element.y - first.y, element.width));
    assert_eq!(
        positions,
        [
            (0.0, 0.0, 100.0),
            (0.0, 10.0, 100.0),
            (110.0, 0.0, 100.0),
            (110.0, 10.0, 100.0),
        ]
    );
    Ok(())
}
