use super::io::read_at;
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Error, ErrorKind},
    path::Path,
};

const MAX_TABLE_BYTES: u64 = 64 * 1024 * 1024;
const SFNT_HEADER_BYTES: usize = 12;
const SFNT_FLAVOR_BYTES: usize = 4;
const SFNT_NUM_TABLES_OFFSET: usize = 4;
const TTC_FACE_COUNT_OFFSET: usize = 8;
const TTC_VERSION_OFFSET: usize = 4;
const TTC_DIRECTORY_OFFSET: u64 = 12;
const TTC_FACE_OFFSET_BYTES: u64 = 4;
const TABLE_RECORD_BYTES: usize = 16;
const TABLE_TAG_BYTES: usize = 4;
const TABLE_OFFSET_FIELD: usize = 8;
const TABLE_LENGTH_FIELD: usize = 12;
#[derive(Clone, Debug)]
pub(super) struct TableRecord {
    pub(super) offset: u64,
    pub(super) length: u32,
}

#[derive(Clone, Debug)]
pub(super) struct FaceDirectory {
    pub(super) tables: BTreeMap<[u8; TABLE_TAG_BYTES], TableRecord>,
    pub(super) file_length: u64,
}

fn read_u16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[TABLE_TAG_BYTES - 1]])
}

fn selected_face_offset(
    file: &mut File,
    file_length: u64,
    header: &[u8],
    face_index: u32,
) -> std::io::Result<u64> {
    if &header[..SFNT_FLAVOR_BYTES] == b"ttcf" {
        ttc_face_offset(file, file_length, header, face_index)
    } else if face_index == 0 {
        Ok(0)
    } else {
        Err(Error::new(
            ErrorKind::InvalidData,
            "non-TTC face index out of range",
        ))
    }
}

fn ttc_face_offset(
    file: &mut File,
    file_length: u64,
    header: &[u8],
    face_index: u32,
) -> std::io::Result<u64> {
    if !matches!(
        &header[TTC_VERSION_OFFSET..TTC_FACE_COUNT_OFFSET],
        b"\0\x01\0\0" | b"\0\x02\0\0"
    ) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "unsupported TTC version",
        ));
    }
    let count = read_u32(&header[TTC_FACE_COUNT_OFFSET..SFNT_HEADER_BYTES]);
    let offsets = ttc_offsets(file, file_length, count)?;
    if count == 0 || face_index >= count {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "TTC face index out of range",
        ));
    }
    let start = usize::try_from(u64::from(face_index) * TTC_FACE_OFFSET_BYTES)
        .map_err(|_| Error::new(ErrorKind::InvalidData, "TTC face offset overflow"))?;
    Ok(u64::from(read_u32(
        &offsets[start..start + TABLE_TAG_BYTES],
    )))
}

fn ttc_offsets(file: &mut File, file_length: u64, count: u32) -> std::io::Result<Vec<u8>> {
    let offsets_length = usize::try_from(
        u64::from(count)
            .checked_mul(TTC_FACE_OFFSET_BYTES)
            .ok_or_else(|| Error::new(ErrorKind::InvalidData, "TTC offset overflow"))?,
    )
    .map_err(|_| Error::new(ErrorKind::InvalidData, "TTC offset length overflow"))?;
    read_at(file, file_length, TTC_DIRECTORY_OFFSET, offsets_length)
}

fn table_directory(
    file: &mut File,
    file_length: u64,
    face_offset: u64,
) -> std::io::Result<BTreeMap<[u8; TABLE_TAG_BYTES], TableRecord>> {
    let sfnt = read_at(file, file_length, face_offset, SFNT_HEADER_BYTES)?;
    if !matches!(
        &sfnt[..SFNT_FLAVOR_BYTES],
        b"\0\x01\0\0" | b"OTTO" | b"true" | b"typ1"
    ) {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "unsupported SFNT flavor",
        ));
    }
    let count = usize::from(read_u16(
        &sfnt[SFNT_NUM_TABLES_OFFSET..SFNT_NUM_TABLES_OFFSET + 2],
    ));
    if count == 0 {
        return Err(Error::new(ErrorKind::InvalidData, "empty SFNT directory"));
    }
    let records = read_at(
        file,
        file_length,
        face_offset + SFNT_HEADER_BYTES as u64,
        count * TABLE_RECORD_BYTES,
    )?;
    parse_table_records(file_length, records)
}

fn parse_table_records(
    file_length: u64,
    records: Vec<u8>,
) -> std::io::Result<BTreeMap<[u8; TABLE_TAG_BYTES], TableRecord>> {
    let mut tables = BTreeMap::new();
    let mut total_table_bytes = 0_u64;
    for record in records.chunks_exact(TABLE_RECORD_BYTES) {
        insert_table_record(&mut tables, &mut total_table_bytes, file_length, record)?;
    }
    Ok(tables)
}

fn insert_table_record(
    tables: &mut BTreeMap<[u8; TABLE_TAG_BYTES], TableRecord>,
    total_table_bytes: &mut u64,
    file_length: u64,
    record: &[u8],
) -> std::io::Result<()> {
    let mut tag = [0; TABLE_TAG_BYTES];
    tag.copy_from_slice(&record[..TABLE_TAG_BYTES]);
    let table = TableRecord {
        offset: u64::from(read_u32(&record[TABLE_OFFSET_FIELD..TABLE_LENGTH_FIELD])),
        length: read_u32(&record[TABLE_LENGTH_FIELD..TABLE_RECORD_BYTES]),
    };
    let end = table
        .offset
        .checked_add(u64::from(table.length))
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "table offset overflow"))?;
    /* WHY: 重複・重畳した table でも callback の累積確保量を bounded にする。 */
    *total_table_bytes = total_table_bytes
        .checked_add(u64::from(table.length))
        .filter(|bytes| *bytes <= MAX_TABLE_BYTES)
        .ok_or_else(|| Error::new(ErrorKind::OutOfMemory, "table bytes exceed cache bound"))?;
    if end > file_length || tables.insert(tag, table).is_some() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "invalid SFNT table directory",
        ));
    }
    Ok(())
}

pub(super) fn directory(path: &Path, face_index: u32) -> std::io::Result<FaceDirectory> {
    #[cfg(all(test, target_os = "macos"))]
    crate::markdown::svg_rasterize::font::observation::count(
        crate::markdown::svg_rasterize::font::observation::Metric::LazyDirectoryOpenAttempt,
    );
    let mut file = File::open(path)?;
    let file_length = file.metadata()?.len();
    let header = read_at(&mut file, file_length, 0, SFNT_HEADER_BYTES)?;
    let face_offset = selected_face_offset(&mut file, file_length, &header, face_index)?;
    let tables = table_directory(&mut file, file_length, face_offset)?;
    Ok(FaceDirectory {
        tables,
        file_length,
    })
}
