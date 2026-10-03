#[path = "svg_rasterize_text_cmap_cache.rs"]
mod cmap_cache;
#[path = "svg_rasterize_text_file_generation.rs"]
mod file_generation;
#[path = "svg_rasterize_text_glyph_cache.rs"]
mod glyph_cache;
#[path = "svg_rasterize_text_stamp_batch.rs"]
mod stamp_batch;
pub(super) use file_generation::{
    FontSourceGeneration, font_source_generation, observe_file_face_generation,
};
pub(super) use glyph_cache::cached_font_has_char_with_generation;
pub(super) use stamp_batch::{memo_usable, with_validated_stamp_batch};
#[path = "svg_rasterize_text_font_score.rs"]
mod score;
use resvg::usvg;
use score::fallback_face_attribute_score;
#[cfg(test)]
use score::{css_stretch_match_rank, css_style_match_rank, fallback_attribute_score};
pub(super) fn matching_font_face(
    database: &usvg::fontdb::Database,
    font_family: &str,
    font_weight: u16,
    italic: bool,
) -> Option<usvg::fontdb::ID> {
    let names = css_font_family_names(font_family);
    let families = names
        .iter()
        .map(|name| fontdb_family(name))
        .collect::<Vec<_>>();
    database.query(&usvg::fontdb::Query {
        families: &families,
        weight: usvg::fontdb::Weight(font_weight),
        stretch: usvg::fontdb::Stretch::Normal,
        style: if italic {
            usvg::fontdb::Style::Italic
        } else {
            usvg::fontdb::Style::Normal
        },
    })
}
#[cfg(test)]
pub(super) fn font_runs(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    text: &str,
) -> Vec<(usvg::fontdb::ID, String)> {
    let mut runs: Vec<(usvg::fontdb::ID, String)> = Vec::new();
    for character in text.chars() {
        let face_id = resolved_base_face(database, base_face_id, character);
        append_font_run(&mut runs, face_id, character);
    }
    runs
}
pub(super) fn append_font_run(
    runs: &mut Vec<(usvg::fontdb::ID, String)>,
    face_id: usvg::fontdb::ID,
    character: char,
) {
    if let Some((run_face_id, run)) = runs.last_mut()
        && *run_face_id == face_id
    {
        run.push(character);
    } else {
        runs.push((face_id, character.to_string()));
    }
}
#[cfg(test)]
fn resolved_base_face(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
) -> usvg::fontdb::ID {
    if font_has_char(database, base_face_id, character) {
        return base_face_id;
    }
    let (weight, italic) = database
        .face(base_face_id)
        .map_or((usvg::fontdb::Weight::NORMAL.0, false), |face| {
            (face.weight.0, face.style == usvg::fontdb::Style::Italic)
        });
    matching_fallback_face(database, base_face_id, character, weight, italic)
        .unwrap_or(base_face_id)
}
#[cfg(test)]
pub(super) fn matching_fallback_face(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
) -> Option<usvg::fontdb::ID> {
    matching_fallback_with_probe(
        database,
        base_face_id,
        character,
        requested_weight,
        requested_italic,
        |id, ch| probe_font_has_char(database, id, ch),
    )
    .0
}
pub(super) fn matching_fallback_with_probe(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_weight: u16,
    requested_italic: bool,
    probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    if database.face(base_face_id).is_none() {
        return (None, false);
    }
    scan_fallback_faces(
        database,
        base_face_id,
        character,
        requested_font_style(requested_italic),
        usvg::fontdb::Weight(requested_weight),
        probe,
    )
}

fn scan_fallback_faces(
    database: &usvg::fontdb::Database,
    base_face_id: usvg::fontdb::ID,
    character: char,
    requested_style: usvg::fontdb::Style,
    requested_weight: usvg::fontdb::Weight,
    mut probe: impl FnMut(usvg::fontdb::ID, char) -> Option<bool>,
) -> (Option<usvg::fontdb::ID>, bool) {
    let mut best = None;
    let mut cacheable = true;
    for face in database.faces().filter(|face| face.id != base_face_id) {
        let score = fallback_face_attribute_score(face, requested_style, requested_weight);
        /* WHY: 同点では先のfaceを保ち、選択結果を改善しないfontのfile解析を省く。 */
        if best.is_some_and(|(best_score, _)| score >= best_score) {
            continue;
        }
        let support = probe(face.id, character);
        cacheable &= support.is_some();
        if support == Some(true) {
            /* WHY: 全属性一致の最小scoreより良い候補はないため、後続faceの走査を省く。 */
            if score == (0, 0, 0, 0) {
                return (Some(face.id), cacheable);
            }
            best = Some((score, face.id));
        }
    }
    (best.map(|(_, face_id)| face_id), cacheable)
}
fn requested_font_style(italic: bool) -> usvg::fontdb::Style {
    if italic {
        usvg::fontdb::Style::Italic
    } else {
        usvg::fontdb::Style::Normal
    }
}
#[cfg(test)]
pub(super) fn font_has_char(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    character: char,
) -> bool {
    probe_font_has_char(database, face_id, character).unwrap_or(false)
}
fn probe_font_has_char(
    database: &usvg::fontdb::Database,
    face_id: usvg::fontdb::ID,
    character: char,
) -> Option<bool> {
    database
        .with_face_data(face_id, |data, face_index| {
            /* WHY: glyphの有無だけを調べるため、不要なGSUB/GPOS coverage構築を省く。 */
            rustybuzz::ttf_parser::Face::parse(data, face_index)
                .ok()
                .map(|face| face.glyph_index(character).is_some())
        })
        .flatten()
}
fn css_font_family_names(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(|name| name.trim_matches(['\'', '"']).to_string())
        .filter(|name| !name.is_empty())
        .collect()
}
pub(super) fn fontdb_family(name: &str) -> usvg::fontdb::Family<'_> {
    match name.to_ascii_lowercase().as_str() {
        "serif" => usvg::fontdb::Family::Serif,
        "sans-serif" | "system-ui" => usvg::fontdb::Family::SansSerif,
        "cursive" => usvg::fontdb::Family::Cursive,
        "fantasy" => usvg::fontdb::Family::Fantasy,
        "monospace" => usvg::fontdb::Family::Monospace,
        _ => usvg::fontdb::Family::Name(name),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        css_stretch_match_rank, css_style_match_rank, fallback_attribute_score, font_has_char,
        matching_fallback_face, matching_font_face,
    };
    use crate::markdown::svg_rasterize::font::{bundled_font_db, html_font_db_for_text};
    use resvg::usvg;
    use resvg::usvg::fontdb::{Database, ID, Source, Stretch, Style, Weight};
    use std::sync::Arc;

    fn shaping_face_has_char(database: &Database, face_id: ID, character: char) -> bool {
        database
            .with_face_data(face_id, |data, face_index| {
                rustybuzz::Face::from_slice(data, face_index)
                    .is_some_and(|face| face.glyph_index(character).is_some())
            })
            .unwrap_or(false)
    }

    fn full_scan_shaping_fallback(
        database: &Database,
        base_face_id: ID,
        character: char,
        requested_weight: u16,
        requested_italic: bool,
    ) -> Option<ID> {
        database.face(base_face_id)?;
        let requested_style = if requested_italic {
            Style::Italic
        } else {
            Style::Normal
        };
        database
            .faces()
            .filter(|face| face.id != base_face_id)
            .filter(|face| shaping_face_has_char(database, face.id, character))
            .min_by_key(|face| {
                fallback_attribute_score(
                    face.style,
                    face.weight,
                    face.stretch,
                    requested_style,
                    Weight(requested_weight),
                    Stretch::Normal,
                )
            })
            .map(|face| face.id)
    }

    struct FallbackFixture {
        database: Database,
        base: ID,
        regular_first: ID,
        italic_first: ID,
        bold: ID,
    }

    fn bundled_latin_face() -> Result<usvg::fontdb::FaceInfo, String> {
        let database = bundled_font_db();
        let face_id = matching_font_face(&database, "Noto Sans", 400, false)
            .ok_or("bundled Latin face must exist")?;
        database
            .face(face_id)
            .cloned()
            .ok_or("bundled face must exist".to_string())
    }

    fn duplicate_sfnt_as_two_face_ttc(sfnt: &[u8]) -> Result<Vec<u8>, String> {
        let table_count = sfnt_table_count(sfnt)?;
        let directory_end = table_count
            .checked_mul(16)
            .and_then(|length| length.checked_add(12))
            .ok_or("SFNT table directory length overflowed")?;
        if sfnt.len() < directory_end {
            return Err("SFNT table directory is truncated".to_string());
        }
        let capacity = sfnt
            .len()
            .checked_add(20)
            .ok_or("TTC fixture length overflowed")?;
        let mut ttc = Vec::with_capacity(capacity);
        ttc.extend_from_slice(b"ttcf");
        ttc.extend_from_slice(&0x0001_0000_u32.to_be_bytes());
        ttc.extend_from_slice(&2_u32.to_be_bytes());
        ttc.extend_from_slice(&20_u32.to_be_bytes());
        ttc.extend_from_slice(&20_u32.to_be_bytes());
        ttc.extend_from_slice(sfnt);
        relocate_ttc_table_offsets(&mut ttc, table_count)?;
        Ok(ttc)
    }

    fn sfnt_table_count(sfnt: &[u8]) -> Result<usize, String> {
        let count = sfnt.get(4..6).ok_or("SFNT table count must exist")?;
        Ok(u16::from_be_bytes([count[0], count[1]]) as usize)
    }

    fn relocate_ttc_table_offsets(ttc: &mut [u8], table_count: usize) -> Result<(), String> {
        for index in 0..table_count {
            let offset = 20 + 12 + index * 16 + 8;
            let table_offset = ttc
                .get(offset..offset + 4)
                .ok_or("SFNT table offset must exist")?;
            let absolute = u32::from_be_bytes([
                table_offset[0],
                table_offset[1],
                table_offset[2],
                table_offset[3],
            ])
            .checked_add(20)
            .ok_or("TTC table offset overflowed")?;
            ttc.get_mut(offset..offset + 4)
                .ok_or("TTC table offset must exist")?
                .copy_from_slice(&absolute.to_be_bytes());
        }
        Ok(())
    }

    fn add_fallback_fixture_face(
        database: &mut Database,
        face: &usvg::fontdb::FaceInfo,
        weight: u16,
        style: Style,
        stretch: Stretch,
        corrupt: bool,
    ) -> ID {
        let mut candidate = face.clone();
        candidate.weight = Weight(weight);
        candidate.style = style;
        candidate.stretch = stretch;
        if corrupt {
            candidate.source = Source::Binary(Arc::new(vec![0, 1, 2, 3]));
        }
        database.push_face_info(candidate)
    }

    fn fallback_fixture() -> Result<FallbackFixture, String> {
        let face = bundled_latin_face()?;
        let mut database = Database::new();
        let mut add = |weight, style, stretch, corrupt| {
            add_fallback_fixture_face(&mut database, &face, weight, style, stretch, corrupt)
        };
        let base = add(400, Style::Normal, Stretch::Normal, false);
        let _unsupported_regular = add(400, Style::Normal, Stretch::Normal, true);
        let regular_first = add(400, Style::Normal, Stretch::Normal, false);
        let _regular_tie = add(400, Style::Normal, Stretch::Normal, false);
        let _condensed_italic = add(700, Style::Italic, Stretch::Condensed, false);
        let _unsupported_italic = add(700, Style::Italic, Stretch::Normal, true);
        let italic_first = add(700, Style::Italic, Stretch::Normal, false);
        let _italic_tie = add(700, Style::Italic, Stretch::Normal, false);
        let bold = add(700, Style::Normal, Stretch::Normal, false);
        Ok(FallbackFixture {
            database,
            base,
            regular_first,
            italic_first,
            bold,
        })
    }

    fn assert_fallback_expected_ids(fixture: &FallbackFixture) {
        for (weight, italic, expected) in [
            (400, false, fixture.regular_first),
            (700, false, fixture.bold),
            (700, true, fixture.italic_first),
            (400, true, fixture.italic_first),
        ] {
            assert_eq!(
                matching_fallback_face(&fixture.database, fixture.base, 'A', weight, italic),
                Some(expected)
            );
        }
    }

    fn assert_fallback_matches_old_scan(database: &Database, base: ID) {
        for (weight, italic) in [(400, false), (700, false), (400, true), (700, true)] {
            for character in ['A', 'é', '日', '😀', '\u{10ffff}'] {
                assert_eq!(
                    matching_fallback_face(database, base, character, weight, italic),
                    full_scan_shaping_fallback(database, base, character, weight, italic)
                );
            }
        }
    }

    #[test]
    fn score_pruned_fallback_preserves_full_scan_order_and_unsupported_candidates()
    -> Result<(), String> {
        let fixture = fallback_fixture()?;
        assert_fallback_expected_ids(&fixture);
        assert_fallback_matches_old_scan(&fixture.database, fixture.base);
        assert_eq!(
            matching_fallback_face(&fixture.database, fixture.base, '\u{10ffff}', 400, false),
            None
        );
        assert_eq!(
            matching_fallback_face(&fixture.database, ID::dummy(), 'A', 400, false),
            None
        );
        Ok(())
    }

    fn assert_latin_glyph_predicate(database: &Database, base: ID) {
        assert!(font_has_char(database, base, 'A'));
        assert!(!font_has_char(database, base, '日'));
        assert!(!font_has_char(database, base, '\u{10ffff}'));
        for character in ['A', 'é', '日', '😀', '\u{10ffff}'] {
            assert_eq!(
                font_has_char(database, base, character),
                shaping_face_has_char(database, base, character)
            );
        }
    }

    fn assert_cjk_fallback_selection(database: &Database, base: ID) -> Result<(), String> {
        for (weight, italic) in [(400, false), (700, false), (700, true)] {
            let fallback = matching_fallback_face(database, base, '日', weight, italic)
                .ok_or("HTML Japanese fallback face must exist")?;
            assert_ne!(fallback, base);
            assert!(font_has_char(database, fallback, '日'));
            assert_eq!(
                Some(fallback),
                full_scan_shaping_fallback(database, base, '日', weight, italic)
            );
            for character in ['A', '日', '😀', '\u{10ffff}'] {
                assert_eq!(
                    font_has_char(database, fallback, character),
                    shaping_face_has_char(database, fallback, character)
                );
            }
        }
        Ok(())
    }

    fn assert_collection_glyph_predicate(database: &Database) {
        for face in database.faces().filter(|face| face.index > 0) {
            let is_collection = database
                .with_face_data(face.id, |data, _| data.starts_with(b"ttcf"))
                .unwrap_or(false);
            if is_collection {
                for character in ['A', '日', '\u{10ffff}'] {
                    assert_eq!(
                        font_has_char(database, face.id, character),
                        shaping_face_has_char(database, face.id, character)
                    );
                }
                break;
            }
        }
    }

    #[test]
    fn glyph_probe_preserves_latin_cjk_and_styled_fallback_selection() -> Result<(), String> {
        let bundled = bundled_font_db();
        let base = matching_font_face(&bundled, "Noto Sans", 400, false)
            .ok_or("bundled Latin face must exist")?;
        assert_latin_glyph_predicate(&bundled, base);
        let html = html_font_db_for_text("Noto Sans", "日本😀");
        let base = matching_font_face(&html, "Noto Sans", 400, false)
            .ok_or("HTML Latin face must exist")?;
        assert_cjk_fallback_selection(&html, base)?;
        assert_collection_glyph_predicate(&html);
        Ok(())
    }

    #[test]
    fn glyph_probe_matches_shaping_for_bundled_ttc_face_data() -> Result<(), String> {
        let bundled = bundled_font_db();
        let face_id = matching_font_face(&bundled, "Noto Sans", 400, false)
            .ok_or("bundled Latin face must exist")?;
        let sfnt = bundled
            .with_face_data(face_id, |data, _| data.to_vec())
            .ok_or("bundled Latin face bytes must exist")?;
        let ttc = duplicate_sfnt_as_two_face_ttc(&sfnt)?;
        let mut collection_face = bundled
            .face(face_id)
            .cloned()
            .ok_or("bundled face must exist")?;
        collection_face.index = 1;
        collection_face.source = Source::Binary(Arc::new(ttc));
        let mut database = Database::new();
        let collection_id = database.push_face_info(collection_face);

        assert!(font_has_char(&database, collection_id, 'A'));
        assert!(!font_has_char(&database, collection_id, '\u{10ffff}'));
        for character in ['A', '\u{10ffff}'] {
            assert_eq!(
                font_has_char(&database, collection_id, character),
                shaping_face_has_char(&database, collection_id, character)
            );
        }
        assert_collection_glyph_predicate(&database);
        Ok(())
    }

    #[test]
    fn ttc_fixture_rejects_missing_and_truncated_sfnt_directories() {
        assert!(duplicate_sfnt_as_two_face_ttc(&[]).is_err());
        assert!(duplicate_sfnt_as_two_face_ttc(&[0, 1, 0, 0, 0, 1]).is_err());
    }

    #[test]
    fn ttc_fixture_rejects_table_offset_overflow() {
        let mut ttc = vec![0; 44];
        ttc[40..44].copy_from_slice(&u32::MAX.to_be_bytes());

        assert!(relocate_ttc_table_offsets(&mut ttc, 1).is_err());
    }

    #[test]
    fn glyph_probe_rejects_missing_face_corrupt_data_and_invalid_collection_index()
    -> Result<(), String> {
        let bundled = bundled_font_db();
        let base = matching_font_face(&bundled, "Noto Sans", 400, false)
            .ok_or("bundled Latin face must exist")?;
        let face = bundled
            .face(base)
            .cloned()
            .ok_or("bundled face must exist")?;
        let mut database = Database::new();
        assert!(!font_has_char(&database, ID::dummy(), 'A'));

        let mut corrupt = face.clone();
        corrupt.source = Source::Binary(Arc::new(vec![0, 1, 2, 3]));
        let corrupt_id = database.push_face_info(corrupt);
        assert!(!font_has_char(&database, corrupt_id, 'A'));

        let mut invalid_index = face;
        invalid_index.index = u32::MAX;
        let invalid_id = database.push_face_info(invalid_index);
        assert!(!font_has_char(&database, invalid_id, 'A'));
        for identifier in [ID::dummy(), corrupt_id, invalid_id] {
            assert_eq!(
                font_has_char(&database, identifier, 'A'),
                shaping_face_has_char(&database, identifier, 'A')
            );
        }
        assert_collection_glyph_predicate(&database);
        Ok(())
    }

    #[test]
    fn styled_cjk_fallback_prefers_the_exact_style_and_weight_face() {
        let styled = fallback_attribute_score(
            Style::Italic,
            Weight::BOLD,
            Stretch::Normal,
            Style::Italic,
            Weight::BOLD,
            Stretch::Normal,
        );
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Italic,
            Weight::BOLD,
            Stretch::Normal,
        );

        assert!(styled < regular);
    }

    #[test]
    fn non_exact_weight_uses_css_directional_matching() {
        let bold = fallback_attribute_score(
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
            Style::Normal,
            Weight(600),
            Stretch::Normal,
        );
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            Weight(600),
            Stretch::Normal,
        );

        assert!(bold < regular);
    }

    #[test]
    fn css_weight_matching_prefers_four_hundred_for_a_five_hundred_request() {
        let bold = fallback_attribute_score(
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
            Style::Normal,
            Weight(500),
            Stretch::Normal,
        );
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            Weight(500),
            Stretch::Normal,
        );

        assert!(regular < bold);
    }

    #[test]
    fn non_exact_stretch_uses_css_directional_matching() {
        assert!(
            css_stretch_match_rank(Stretch::SemiCondensed, Stretch::Normal)
                < css_stretch_match_rank(Stretch::SemiExpanded, Stretch::Normal)
        );
        assert!(
            css_stretch_match_rank(Stretch::Expanded, Stretch::SemiExpanded)
                < css_stretch_match_rank(Stretch::Normal, Stretch::SemiExpanded)
        );
    }

    #[test]
    fn css_font_matching_covers_stretch_extremes_and_style_fallback() {
        for stretch in [
            Stretch::UltraCondensed,
            Stretch::ExtraCondensed,
            Stretch::Condensed,
            Stretch::SemiCondensed,
            Stretch::Normal,
            Stretch::SemiExpanded,
            Stretch::Expanded,
            Stretch::ExtraExpanded,
            Stretch::UltraExpanded,
        ] {
            let _ = css_stretch_match_rank(stretch, Stretch::Normal);
        }
        assert_eq!(css_style_match_rank(Style::Oblique, Style::Oblique), 0);
        assert_eq!(css_style_match_rank(Style::Italic, Style::Oblique), 1);
        assert_eq!(css_style_match_rank(Style::Oblique, Style::Normal), 3);
        assert_eq!(css_style_match_rank(Style::Normal, Style::Oblique), 2);
    }

    #[test]
    fn italic_request_prefers_oblique_before_normal() {
        assert!(
            css_style_match_rank(Style::Oblique, Style::Italic)
                < css_style_match_rank(Style::Normal, Style::Italic)
        );
    }

    #[test]
    fn fallback_matching_prefers_normal_stretch_regular_over_condensed_bold() {
        let regular = fallback_attribute_score(
            Style::Normal,
            Weight::NORMAL,
            Stretch::Normal,
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
        );
        let condensed_bold = fallback_attribute_score(
            Style::Normal,
            Weight::BOLD,
            Stretch::Condensed,
            Style::Normal,
            Weight::BOLD,
            Stretch::Normal,
        );

        assert!(regular < condensed_bold);
    }
}
