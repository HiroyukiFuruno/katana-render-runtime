use super::{Ordering, load};
use crate::markdown::svg_rasterize::font_file_fixture::FONT_BYTES;
use skrifa::raw::TableProvider;
use std::io::ErrorKind;

#[cfg(any(target_os = "macos", target_os = "linux"))]
#[test]
fn lazy_font_table_read_reports_a_non_seekable_source() -> Result<(), Box<dyn std::error::Error>> {
    let (reader, _writer) = nix::unistd::pipe()?;
    let mut file = std::fs::File::from(reader);
    let file_length = file.metadata()?.len();
    let result = super::io::read_at(&mut file, file_length, 0, 0);
    assert_eq!(
        result.err().and_then(|error| error.raw_os_error()),
        Some(nix::errno::Errno::ESPIPE as i32)
    );
    Ok(())
}

#[test]
fn truncated_font_poisons_an_existing_lazy_table_source() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("font.ttf");
    std::fs::write(&path, FONT_BYTES)?;
    let baseline = load(&path, 0)?;
    assert!(baseline.font.tables().name().is_ok());

    let stale = load(&path, 0)?;
    std::fs::write(&path, [])?;
    assert!(stale.font.tables().name().is_err());
    assert!(stale.poisoned.load(Ordering::Acquire));

    std::fs::write(&path, FONT_BYTES)?;
    let fresh = load(&path, 0)?;
    assert!(fresh.font.tables().name().is_ok());
    assert!(!fresh.poisoned.load(Ordering::Acquire));
    Ok(())
}

const SFNT_SIGNATURE_BYTES: usize = 4;

#[test]
fn lazy_table_read_rejects_invalid_ranges_and_truncated_file()
-> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("font.ttf");
    std::fs::write(&path, FONT_BYTES)?;
    let mut file = std::fs::File::open(&path)?;
    let length = file.metadata()?.len();
    let valid = super::io::read_at(&mut file, length, 0, SFNT_SIGNATURE_BYTES)?;
    assert_eq!(valid, FONT_BYTES[..SFNT_SIGNATURE_BYTES]);
    for (offset, kind) in [
        (u64::MAX, ErrorKind::InvalidData),
        (length, ErrorKind::UnexpectedEof),
    ] {
        let error = super::io::read_at(&mut file, length, offset, 1)
            .err()
            .map(|error| error.kind());
        assert_eq!(error, Some(kind), "offset {offset}");
    }
    std::fs::write(&path, [])?;
    let error = super::io::read_at(&mut file, length, 0, SFNT_SIGNATURE_BYTES)
        .err()
        .map(|error| error.kind());
    assert_eq!(error, Some(ErrorKind::UnexpectedEof));
    Ok(())
}
