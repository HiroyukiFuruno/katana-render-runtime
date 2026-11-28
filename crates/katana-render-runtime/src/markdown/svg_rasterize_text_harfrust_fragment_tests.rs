use super::{FontShape, collect_character_metrics, shape_font};

const FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/NotoSans-Regular.ttf");
const TEST_FONT_SIZE: f32 = 16.0;
const TEST_FONT_WEIGHT: u16 = 400;
const SUBPIXEL_UNITS_PER_PX: f32 = 64.0;
const TEST_FRACTIONAL_ADVANCE: f32 = 0.5001;
/* WHY: 同梱フォントの実バイトを Chrome FontFace に読み込ませた独立計測値。 */
const CHROME_AV_JOINED: f32 = 19.1875;
const CHROME_AV_SPLIT: [f32; 2] = [9.59375, 9.609375];
const CHROME_AVA_JOINED: f32 = 28.78125;
const CHROME_AVA_SPLIT: [f32; 2] = [9.59375, 19.1875];
const CHROME_FISH_JOINED: f32 = 104.890625;
const CHROME_FISH_SPLIT: [f32; 2] = [24.953125, 79.9375];
const CHROME_ZEBRA_JOINED: f32 = 96.21875;
const CHROME_ZEBRA_SPLIT: [f32; 2] = [42.484375, 53.734375];
type WholeRunMetrics = (Vec<(f32, f32)>, Vec<usize>);

fn utf16_endpoints(text: &str) -> Vec<usize> {
    let mut endpoint = 0;
    text.chars()
        .map(|character| {
            endpoint += character.len_utf16();
            endpoint
        })
        .collect()
}

fn fragment_character_ends(text: &str, fragment_ends_utf16: &[usize]) -> Option<Vec<usize>> {
    let endpoints = utf16_endpoints(text);
    if fragment_ends_utf16.is_empty() {
        return None;
    }
    let mut start = 0;
    let mut ends = Vec::with_capacity(fragment_ends_utf16.len());
    for &end in fragment_ends_utf16 {
        let finish = endpoints.binary_search(&end).ok()? + 1;
        if finish <= start {
            return None;
        }
        start = finish;
        ends.push(finish);
    }
    (start == endpoints.len()).then_some(ends)
}

fn fragment_cluster_boundaries(
    text: &str,
    fragment_character_ends: &[usize],
    cluster_starts: &[usize],
) -> Option<()> {
    let byte_ends = text
        .char_indices()
        .map(|(start, character)| start + character.len_utf8())
        .collect::<Vec<_>>();
    let clusters = validated_cluster_starts(text, cluster_starts)?;
    for &end in fragment_character_ends {
        let boundary = *byte_ends.get(end.checked_sub(1)?)?;
        if fragment_boundary_crosses_cluster(boundary, text.len(), &clusters) {
            return None;
        }
    }
    Some(())
}

fn validated_cluster_starts(text: &str, cluster_starts: &[usize]) -> Option<Vec<usize>> {
    let mut clusters = cluster_starts.to_vec();
    clusters.sort_unstable();
    clusters.dedup();
    (clusters.first().copied() == Some(0)
        && clusters
            .iter()
            .all(|cluster| *cluster < text.len() && text.is_char_boundary(*cluster)))
    .then_some(clusters)
}

fn fragment_boundary_crosses_cluster(boundary: usize, text_len: usize, clusters: &[usize]) -> bool {
    clusters.iter().enumerate().any(|(index, start)| {
        let finish = clusters.get(index + 1).copied().unwrap_or(text_len);
        start < &boundary && boundary < finish
    })
}

fn quantized_fragment_advances(
    text: &str,
    fragment_ends_utf16: &[usize],
    metrics: &[(f32, f32)],
    cluster_starts: &[usize],
) -> Option<Vec<f32>> {
    if !super::is_left_to_right(text) || utf16_endpoints(text).len() != metrics.len() {
        return None;
    }
    let ends = fragment_character_ends(text, fragment_ends_utf16)?;
    fragment_cluster_boundaries(text, &ends, cluster_starts)?;
    let mut start = 0;
    ends.into_iter().try_fold(Vec::new(), |mut widths, finish| {
        let width = metrics[start..finish].iter().try_fold(0.0, |sum, metric| {
            (metric.0.is_finite() && metric.0 >= 0.0).then_some(sum + metric.0)
        })?;
        start = finish;
        let quantized = (width * SUBPIXEL_UNITS_PER_PX).ceil() / SUBPIXEL_UNITS_PER_PX;
        quantized
            .is_finite()
            .then(|| widths.push(quantized))
            .map(|()| widths)
    })
}

fn whole_run_metrics(text: &str) -> Option<WholeRunMetrics> {
    let face = harfrust::font::Font::new(FONT_BYTES.to_vec(), 0)?;
    let font = FontShape::new(TEST_FONT_SIZE, TEST_FONT_WEIGHT, false);
    let glyphs = shape_font(&face, text, font, None)?;
    let metrics = collect_character_metrics(&glyphs, text, 1.0 / super::HARFRUST_SCALE)?;
    let clusters = glyphs
        .glyph_infos()
        .iter()
        .map(|info| info.cluster as usize)
        .collect();
    Some((metrics, clusters))
}

fn assert_noto_chrome_widths(
    text: &str,
    fragment_ends_utf16: &[usize],
    chrome_joined: f32,
    chrome_fragments: &[f32],
) -> Result<(), String> {
    let (metrics, clusters) = whole_run_metrics(text).ok_or("whole-run HarfRust shaping failed")?;
    let joined = metrics.iter().map(|metric| metric.0).sum::<f32>();
    let joined = (joined * SUBPIXEL_UNITS_PER_PX).ceil() / SUBPIXEL_UNITS_PER_PX;
    let fragments = quantized_fragment_advances(text, fragment_ends_utf16, &metrics, &clusters)
        .ok_or_else(|| format!("invalid cluster or UTF-16 fragment boundary for {text:?}"))?;
    assert_eq!(joined, chrome_joined, "joined Chrome width for {text:?}");
    assert_eq!(
        fragments, chrome_fragments,
        "split Chrome widths for {text:?}"
    );
    Ok(())
}

#[test]
fn bundled_noto_whole_run_matches_chrome_joined_and_fragment_widths() -> Result<(), String> {
    assert_noto_chrome_widths("AV", &[1, 2], CHROME_AV_JOINED, &CHROME_AV_SPLIT)?;
    assert_noto_chrome_widths("AVA", &[1, 3], CHROME_AVA_JOINED, &CHROME_AVA_SPLIT)?;
    assert_noto_chrome_widths(
        "-int sizeInFeet",
        &[4, 15],
        CHROME_FISH_JOINED,
        &CHROME_FISH_SPLIT,
    )?;
    assert_noto_chrome_widths(
        "+bool is_wild",
        &[5, 13],
        CHROME_ZEBRA_JOINED,
        &CHROME_ZEBRA_SPLIT,
    )
}

#[test]
fn utf16_surrogate_and_cluster_crossing_boundaries_fail_closed() {
    let text = "A😀B";
    let metrics = [(1.0, 0.0), (2.0, 0.0), (3.0, 0.0)];
    let clusters = [0, 1, 5];
    assert!(quantized_fragment_advances(text, &[1, 3, 4], &metrics, &clusters).is_some());
    assert!(quantized_fragment_advances(text, &[2, 4], &metrics, &clusters).is_none());
    assert!(quantized_fragment_advances("fi", &[1, 2], &metrics[..2], &[0]).is_none());
    assert!(quantized_fragment_advances("AB", &[1], &metrics[..2], &[0, 1]).is_none());
    assert!(quantized_fragment_advances("אב", &[1, 2], &metrics[..2], &[0, 2]).is_none());
}

#[test]
fn fragment_ceiling_preserves_full_run_metrics() {
    let metrics = [
        (TEST_FRACTIONAL_ADVANCE, 0.0),
        (TEST_FRACTIONAL_ADVANCE, 0.0),
    ];
    let joined = quantized_fragment_advances("AB", &[2], &metrics, &[0, 1]);
    let split = quantized_fragment_advances("AB", &[1, 2], &metrics, &[0, 1]);
    assert_eq!(joined, Some(vec![65.0 / SUBPIXEL_UNITS_PER_PX]));
    assert_eq!(
        split,
        Some(vec![
            33.0 / SUBPIXEL_UNITS_PER_PX,
            33.0 / SUBPIXEL_UNITS_PER_PX
        ])
    );
}
