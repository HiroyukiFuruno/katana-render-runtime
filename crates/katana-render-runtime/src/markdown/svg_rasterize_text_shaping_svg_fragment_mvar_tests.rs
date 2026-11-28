use super::{Database, FontShape, SvgRasterizeError, layout_for_face};
use read_fonts::FontRef;

const ROBOTO_FLEX: &[u8] = include_bytes!(
    "../../../../tests/fixtures/svg-engine/upstream/resvg/tests/fonts/RobotoFlex.subset.ttf"
);
const HHEA_ASCENDER: usize = 4;
const HHEA_DESCENDER: usize = 6;
const OS2_SELECTION: usize = 62;
const OS2_TYPO_ASCENDER: usize = 68;
const OS2_TYPO_DESCENDER: usize = 70;
const OS2_WIN_ASCENT: usize = 74;
const MVAR_FIRST_TAG: usize = 12;
const MVAR_WEIGHT_REGION: usize = 1314;
const MVAR_YTUC_REGION: usize = 1356;
const MVAR_FIRST_DELTA: usize = 6620;
const SIZE: f32 = 16.0;
const MAX_WEIGHT: u16 = 1000;
const TAG_BYTES: usize = 4;
const MAX_REGION_COORDINATE: [u8; 6] = [0, 0, 0x40, 0, 0x40, 0];

#[test]
fn negative_variable_font_layout_ascent_is_rejected_after_shaping() -> Result<(), String> {
    let mut database = Database::new();
    database.load_font_data(font_with_negative_clipping_ascent()?);
    let id = database
        .faces()
        .next()
        .ok_or("Roboto Flex face missing")?
        .id;
    let shape = FontShape::new(SIZE, MAX_WEIGHT, false);
    let Some((metrics, advances)) =
        super::super::harfrust::shape_fragment_run(&database, id, "A", shape)?
    else {
        return Err("Roboto Flex fragment did not shape".to_owned());
    };
    assert_eq!(advances.len(), 1);
    assert!(metrics.ascent < -0.5 && metrics.ascent + metrics.descent > 0.0);
    assert!(matches!(
        layout_for_face(&database, id, id, "A", shape, &["A".to_owned()]),
        Err(SvgRasterizeError::RasterizeFailed(message)) if message == "invalid SVG metrics"
    ));
    Ok(())
}

fn font_with_negative_clipping_ascent() -> Result<Vec<u8>, String> {
    let mut bytes = ROBOTO_FLEX.to_vec();
    let hhea = table_offset(*b"hhea")?;
    let os2 = table_offset(*b"OS/2")?;
    for index in [
        hhea + HHEA_ASCENDER,
        hhea + HHEA_DESCENDER,
        os2 + OS2_SELECTION,
        os2 + OS2_TYPO_ASCENDER,
        os2 + OS2_TYPO_DESCENDER,
        os2 + OS2_WIN_ASCENT,
    ] {
        write(&mut bytes, index, &[0; size_of::<u16>()]);
    }
    move_cap_height_delta(&mut bytes, table_offset(*b"MVAR")?);
    Ok(bytes)
}

fn move_cap_height_delta(bytes: &mut [u8], mvar: usize) {
    /* WHY: このfixtureのcap-height変動をwght軸のWindows clipping ascentへ移す。 */
    assert_eq!(&bytes[mvar + MVAR_FIRST_TAG..][..b"cpht".len()], b"cpht");
    assert_eq!(
        &bytes[mvar + MVAR_YTUC_REGION..][..MAX_REGION_COORDINATE.len()],
        &MAX_REGION_COORDINATE
    );
    write(bytes, mvar + MVAR_FIRST_TAG, b"hcla");
    write(bytes, mvar + MVAR_WEIGHT_REGION, &MAX_REGION_COORDINATE);
    write(
        bytes,
        mvar + MVAR_YTUC_REGION,
        &[0; MAX_REGION_COORDINATE.len()],
    );
    bytes[mvar + MVAR_FIRST_DELTA] = i8::MIN.to_be_bytes()[0];
}

fn table_offset(tag: [u8; TAG_BYTES]) -> Result<usize, String> {
    let font = FontRef::new(ROBOTO_FLEX).map_err(|error| error.to_string())?;
    font.table_directory()
        .table_records()
        .iter()
        .find(|record| record.tag().to_be_bytes() == tag)
        .map(|record| record.offset() as usize)
        .ok_or_else(|| format!("Roboto Flex table missing: {tag:?}"))
}

fn write(bytes: &mut [u8], offset: usize, value: &[u8]) {
    bytes[offset..offset + value.len()].copy_from_slice(value);
}
