use std::{
    fs::File,
    io::{Error, ErrorKind, Read, Seek, SeekFrom},
    path::Path,
};

pub(super) fn read_at(
    file: &mut File,
    file_length: u64,
    offset: u64,
    length: usize,
) -> std::io::Result<Vec<u8>> {
    let end = offset
        .checked_add(length as u64)
        .ok_or_else(|| Error::new(ErrorKind::InvalidData, "offset overflow"))?;
    if end > file_length {
        return Err(Error::new(ErrorKind::UnexpectedEof, "table exceeds file"));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; length];
    file.read_exact(&mut bytes)?;
    Ok(bytes)
}

pub(super) fn read_table(
    path: &Path,
    file_length: u64,
    offset: u64,
    table_length: u32,
) -> std::io::Result<Vec<u8>> {
    #[cfg(all(test, target_os = "macos"))]
    crate::markdown::svg_rasterize::font::observation::count(
        crate::markdown::svg_rasterize::font::observation::Metric::LazyTableOpenAttempt,
    );
    let mut file = File::open(path)?;
    let length = table_length as usize;
    read_at(&mut file, file_length, offset, length)
}
