use super::ResourceBytes;
use std::sync::Arc;

#[test]
fn owned_reads_do_not_create_shared_cache_entries() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("owned-resource");
    std::fs::write(&path, b"\x00\xffpayload")?;
    assert_eq!(ResourceBytes::read_file(&path)?, b"\x00\xffpayload");
    assert!(
        super::CACHE
            .get()
            .and_then(|cache| cache.get(path.as_path()))
            .is_none()
    );
    std::fs::remove_file(&path)?;
    assert!(
        ResourceBytes::read_file(&path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    );
    Ok(())
}

#[test]
fn unchanged_file_shares_snapshot_and_replacement_invalidates()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("asset");
    let replacement = dir.path().join("replacement");
    std::fs::write(&path, b"first")?;
    let first = ResourceBytes::snapshot(&path)?;
    let second = ResourceBytes::snapshot(&path)?;
    assert!(Arc::ptr_eq(&first, &second));
    assert!(Arc::ptr_eq(&first.text_handle()?, &second.text_handle()?));
    std::fs::write(&replacement, b"other")?;
    std::fs::rename(&replacement, &path)?;
    let changed = ResourceBytes::snapshot(&path)?;
    assert_eq!(changed.bytes(), b"other");
    assert_ne!(first.digest(), changed.digest());
    std::fs::remove_file(&path)?;
    assert_eq!(
        ResourceBytes::snapshot(&path)
            .err()
            .map(|error| error.kind()),
        Some(std::io::ErrorKind::NotFound)
    );
    Ok(())
}

#[test]
fn oversized_file_is_read_but_not_retained() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("large");
    std::fs::write(&path, vec![b'x'; super::CACHE_BYTES as usize])?;
    let first = ResourceBytes::snapshot(&path)?;
    let second = ResourceBytes::snapshot(&path)?;
    assert!(!Arc::ptr_eq(&first, &second));
    assert_eq!(first.digest(), second.digest());
    Ok(())
}

#[test]
fn text_snapshot_digest_matches_its_bytes() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("text");
    std::fs::write(&path, b"abc")?;
    let snapshot = ResourceBytes::snapshot(&path)?;
    assert_eq!(snapshot.text()?, "abc");
    assert!(snapshot.digest.get().is_none());
    assert_eq!(ResourceBytes::read_file(&path)?, b"abc");
    assert!(snapshot.digest.get().is_none());
    let digest = snapshot.digest();
    assert_eq!(
        digest,
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert!(std::ptr::eq(digest, snapshot.digest()));
    Ok(())
}

#[test]
fn reads_binary_and_observes_replacement_and_removal() -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("resource");
    std::fs::write(&path, b"\0\xff\x01")?;
    assert_eq!(ResourceBytes::read_file(&path)?, b"\0\xff\x01");
    assert!(
        ResourceBytes::read_text(&path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::InvalidData)
    );
    std::fs::write(&path, b"new")?;
    assert_eq!(ResourceBytes::read_text(&path)?, "new");
    std::fs::remove_file(&path)?;
    assert!(
        ResourceBytes::read_file(&path)
            .is_err_and(|error| error.kind() == std::io::ErrorKind::NotFound)
    );
    Ok(())
}
