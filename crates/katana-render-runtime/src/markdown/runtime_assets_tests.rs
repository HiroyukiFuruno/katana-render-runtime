use super::{
    DRAWIO_JS_CHECKSUM, MERMAID_JS_CHECKSUM, MERMAID_ZENUML_JS_CHECKSUM, RuntimeAsset,
    ZENUML_CORE_JS_CHECKSUM,
};

const PARALLEL_MATERIALIZE_THREADS: usize = 8;

#[test]
fn materialized_paths_are_versioned() {
    let mermaid = RuntimeAsset::mermaid().materialized_path();
    let drawio = RuntimeAsset::drawio().materialized_path();
    let zenuml_core = RuntimeAsset::zenuml_core().materialized_path();

    assert!(mermaid.ends_with("vendor/mermaid/11.15.0/mermaid.min.js"));
    assert!(drawio.ends_with("vendor/drawio/30.0.4/drawio.min.js"));
    assert!(zenuml_core.ends_with("vendor/zenuml-core/3.47.9/zenuml.js"));
}

#[test]
fn pinned_checksums_are_sha256_hex() {
    assert_eq!(MERMAID_JS_CHECKSUM.len(), 64);
    assert_eq!(MERMAID_ZENUML_JS_CHECKSUM.len(), 64);
    assert_eq!(DRAWIO_JS_CHECKSUM.len(), 64);
    assert_eq!(ZENUML_CORE_JS_CHECKSUM.len(), 64);
    assert!(MERMAID_JS_CHECKSUM.chars().all(|it| it.is_ascii_hexdigit()));
    assert!(
        MERMAID_ZENUML_JS_CHECKSUM
            .chars()
            .all(|it| it.is_ascii_hexdigit())
    );
    assert!(DRAWIO_JS_CHECKSUM.chars().all(|it| it.is_ascii_hexdigit()));
    assert!(
        ZENUML_CORE_JS_CHECKSUM
            .chars()
            .all(|it| it.is_ascii_hexdigit())
    );
}

#[test]
fn materialize_writes_missing_asset_file() {
    let path = test_path("missing-mermaid.min.js");
    remove_parent(&path);

    let result = RuntimeAsset::mermaid().materialize_at(path.clone());

    assert!(matches!(result, Ok(written) if written == path));
    assert!(path.exists());
    remove_parent(&path);
}

#[test]
fn materialize_reports_empty_path_and_read_errors() {
    let empty_path = RuntimeAsset::mermaid().materialize_at(std::path::PathBuf::new());
    assert!(matches!(empty_path, Err(error) if error.contains("parent")));

    let path = test_path("runtime-directory");
    let _ = std::fs::remove_dir_all(&path);
    let create_result = std::fs::create_dir_all(&path);
    assert!(create_result.is_ok());

    let read_error = RuntimeAsset::mermaid().materialize_at(path.clone());
    assert!(read_error.is_err());
    let _ = std::fs::remove_dir_all(&path);
    remove_parent(&path);
}

#[test]
fn materialize_replaces_different_existing_asset_file() {
    let path = test_path("stale-mermaid.min.js");
    remove_parent(&path);
    let parent = path.parent();
    assert!(matches!(parent, Some(it) if std::fs::create_dir_all(it).is_ok()));
    let write_result = std::fs::write(&path, b"stale");
    assert!(write_result.is_ok());

    let result = RuntimeAsset::mermaid().materialize_at(path.clone());

    assert!(result.is_ok());
    let stored = std::fs::read(path.clone());
    assert!(matches!(stored, Ok(bytes) if bytes != b"stale"));
    remove_parent(&path);
}

#[test]
fn materialize_keeps_same_existing_asset_file() {
    let path = test_path("current-mermaid.min.js");
    remove_parent(&path);
    let first = RuntimeAsset::mermaid().materialize_at(path.clone());
    assert!(matches!(first, Ok(written) if written == path));

    let second = RuntimeAsset::mermaid().materialize_at(path.clone());

    assert!(matches!(second, Ok(written) if written == path));
    remove_parent(&path);
}

#[test]
fn materialize_is_safe_for_parallel_callers() {
    let path = test_path("parallel-mermaid.min.js");
    remove_parent(&path);
    let handles = parallel_materialize_handles(&path);

    for handle in handles {
        let joined = handle.join();
        assert!(matches!(joined, Ok(Ok(written)) if written == path));
    }
    let stored = std::fs::read(path.clone());
    let asset = RuntimeAsset::mermaid();
    assert!(matches!(stored, Ok(bytes) if bytes.as_slice() == asset.bytes));
    remove_parent(&path);
}

#[test]
fn runtime_asset_error_keeps_io_error_message() {
    let error = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");

    let message = super::runtime_asset_error(error);

    assert_eq!(message, "denied");
}

#[test]
fn remove_parent_accepts_path_without_parent() {
    remove_parent(std::path::Path::new(""));
}

fn parallel_materialize_handles(
    path: &std::path::Path,
) -> Vec<std::thread::JoinHandle<Result<std::path::PathBuf, String>>> {
    (0..PARALLEL_MATERIALIZE_THREADS)
        .map(|_| {
            let thread_path = path.to_path_buf();
            std::thread::spawn(move || RuntimeAsset::mermaid().materialize_at(thread_path))
        })
        .collect()
}

fn test_path(name: &str) -> std::path::PathBuf {
    let slug = name.replace(['.', '/'], "-");
    std::env::temp_dir()
        .join(format!(
            "kdr-runtime-assets-test-{}-{slug}",
            std::process::id()
        ))
        .join(name)
}

fn remove_parent(path: &std::path::Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }
}
