use super::{
    MAPPED_FONT_SESSION_BYTES, eligible_immutable_system_path, map_source, observe_path_stamp,
};
use std::{error::Error, path::Path};

const SYSTEM_FONT: &str = "/System/Library/Fonts/Helvetica.ttc";

#[test]
fn native_source_mapping_requires_an_eligible_system_font_within_budget()
-> Result<(), Box<dyn Error>> {
    let system = Path::new(SYSTEM_FONT).canonicalize()?;
    let length = std::fs::metadata(&system)?.len();
    let stamp = observe_path_stamp(&system).map_err(|()| "system stamp missing")?;
    let is_eligible = eligible_immutable_system_path(&system, length)?;
    let mapped = map_source(&system, &stamp, MAPPED_FONT_SESSION_BYTES);
    assert_eq!(
        mapped
            .as_ref()
            .map(|bytes| bytes.as_ref().as_ref().len() as u64),
        (is_eligible && length <= MAPPED_FONT_SESSION_BYTES).then_some(length)
    );
    assert!(map_source(&system, &stamp, 0).is_none());
    Ok(())
}

#[test]
fn native_source_mapping_rejects_a_writable_copy_or_mismatched_stamp() -> Result<(), Box<dyn Error>>
{
    let system = Path::new(SYSTEM_FONT).canonicalize()?;
    let temporary = tempfile::tempdir()?;
    let writable = temporary.path().join("Helvetica.ttc");
    std::fs::copy(&system, &writable)?;
    let stamp = observe_path_stamp(&writable).map_err(|()| "writable stamp missing")?;
    assert!(map_source(&writable, &stamp, MAPPED_FONT_SESSION_BYTES).is_none());
    assert!(map_source(&system, &stamp, MAPPED_FONT_SESSION_BYTES).is_none());
    Ok(())
}

#[test]
fn concurrent_native_font_sources_share_one_mapping_without_retaining_it()
-> Result<(), Box<dyn Error>> {
    let system = Path::new("/System/Library/Fonts/Monaco.ttf").canonicalize()?;
    let stamp = observe_path_stamp(&system).map_err(|()| "system stamp missing")?;
    let length = std::fs::metadata(&system)?.len();
    if !eligible_immutable_system_path(&system, length)? {
        return Ok(());
    }
    let first = map_source(&system, &stamp, MAPPED_FONT_SESSION_BYTES)
        .ok_or("first native font mapping missing")?;
    let second = map_source(&system, &stamp, MAPPED_FONT_SESSION_BYTES)
        .ok_or("second native font mapping missing")?;
    assert!(std::sync::Arc::ptr_eq(&first, &second));

    let mapping = std::sync::Arc::downgrade(&first);
    drop((first, second));
    assert!(mapping.upgrade().is_none());
    Ok(())
}
