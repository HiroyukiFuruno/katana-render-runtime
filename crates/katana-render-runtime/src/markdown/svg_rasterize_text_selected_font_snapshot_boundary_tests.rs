use super::{
    FileFace, MAPPED_FONT_SESSION_BYTES, MappedFontBytes, copy_bounded, create_bounded_snapshot,
    map_large_source, map_verified_snapshot, open_origin, stamp_open_font_file,
    stamp_path_uncached,
};
use resvg::usvg::fontdb::{Database, Source};
use std::{
    error::Error, fs::File, io::Write, net::Shutdown, os::fd::OwnedFd, os::unix::net::UnixStream,
    path::Path, sync::Arc,
};

type TestResult = Result<(), Box<dyn Error>>;

const CONTENT: &[u8] = b"original";
const LARGE_LENGTH: u64 = MAPPED_FONT_SESSION_BYTES + 1;

#[test]
fn snapshot_streams_when_the_source_does_not_support_reflink() -> TestResult {
    let (reader, mut writer) = UnixStream::pair()?;
    writer.write_all(CONTENT)?;
    writer.shutdown(Shutdown::Write)?;
    let mut source = File::from(OwnedFd::from(reader));
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("snapshot");
    create_bounded_snapshot(&mut source, &target, CONTENT.len() as u64)?;
    assert_eq!(std::fs::read(target)?, CONTENT);
    Ok(())
}

#[test]
fn bounded_copy_copies_the_selected_length_and_stops_after_one_extra_byte() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("origin");
    std::fs::write(&path, CONTENT)?;
    let exact = directory.path().join("exact");
    let over = directory.path().join("over");
    copy_bounded(&mut File::open(&path)?, &exact, CONTENT.len() as u64)?;
    assert_eq!(std::fs::read(exact)?, CONTENT);
    let attempted_length = CONTENT.len() as u64 - 2;
    let result = copy_bounded(&mut File::open(path)?, &over, attempted_length);
    assert_eq!(
        result.err().map(|error| error.kind()),
        Some(std::io::ErrorKind::InvalidData)
    );
    assert_eq!(std::fs::metadata(over)?.len(), attempted_length + 1);
    Ok(())
}

#[test]
fn snapshot_publication_rejects_a_replaced_path_with_an_open_original() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("origin");
    std::fs::write(&path, CONTENT)?;
    let mut origin = File::open(&path)?;
    let scratch = tempfile::tempdir()?;
    let snapshot = scratch.path().join("copy");
    copy_bounded(&mut origin, &snapshot, CONTENT.len() as u64)?;
    std::fs::rename(&path, directory.path().join("old-origin"))?;
    std::fs::write(&path, CONTENT)?;
    let held = stamp_open_font_file(&path, &origin).map_err(|()| "open stamp unavailable")?;
    let current = stamp_path_uncached(&path).map_err(|()| "path stamp unavailable")?;
    assert_ne!(held, current);
    assert!(
        map_verified_snapshot(
            &path,
            &held,
            held.file_length(),
            &origin,
            &snapshot,
            scratch
        )
        .is_none()
    );
    assert!(!snapshot.exists());
    Ok(())
}

#[test]
fn open_source_stamp_rejects_a_directory_handle() -> TestResult {
    let directory = tempfile::tempdir()?;
    let handle = open_origin(directory.path())?;
    assert!(stamp_open_font_file(directory.path(), &handle).is_err());
    Ok(())
}

#[test]
fn large_font_faces_share_one_mapping_in_each_frame() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("large-font");
    std::fs::copy("/System/Library/Fonts/Helvetica.ttc", &path)?;
    File::options()
        .write(true)
        .open(&path)?
        .set_len(LARGE_LENGTH)?;
    let stamp = stamp_path_uncached(&path).map_err(|()| "origin stamp unavailable")?;
    let mapping =
        map_large_source(&path, &stamp, LARGE_LENGTH, &[]).ok_or("large font not mapped")?;
    let frame = [file_face(&path, Arc::clone(&mapping))?];
    let reused = map_large_source(&path, &stamp, LARGE_LENGTH, &frame)
        .ok_or("frame mapping was not reused")?;
    assert!(Arc::ptr_eq(&mapping, &reused));
    let other = directory.path().join("another-font");
    assert!(map_large_source(&other, &stamp, LARGE_LENGTH, &frame).is_none());
    Ok(())
}

fn file_face(path: &Path, mapping: MappedFontBytes) -> Result<FileFace, Box<dyn Error>> {
    let mut database = Database::new();
    database.load_font_source(Source::SharedFile(path.to_path_buf(), mapping));
    let face = database.faces().next().ok_or("font has no valid face")?;
    let stamp = stamp_path_uncached(path).map_err(|()| "origin stamp unavailable")?;
    Ok(FileFace {
        path: path.to_path_buf(),
        name: face.post_script_name.clone(),
        stamp,
        id: face.id,
        database: Arc::new(database),
        name_table: None,
    })
}
