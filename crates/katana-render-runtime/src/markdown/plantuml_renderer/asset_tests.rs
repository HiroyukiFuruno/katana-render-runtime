use super::{
    PLANTUML_DOWNLOAD_URL, PLANTUML_JAR_CHECKSUM, PLANTUML_JAR_VERSION, PlantUmlJarAssetOps,
};
use std::path::Path;

#[test]
fn plantuml_asset_metadata_is_pinned() {
    assert_eq!(PLANTUML_JAR_VERSION, "1.2026.4");
    assert_eq!(PLANTUML_JAR_CHECKSUM.len(), 64);
    assert!(PLANTUML_DOWNLOAD_URL.contains("plantuml-lgpl"));
}

#[test]
fn cache_path_uses_explicit_root_and_pinned_version() {
    assert_eq!(
        PlantUmlJarAssetOps::cache_path(Some(Path::new("/tmp/kdr-cache"))),
        Path::new("/tmp/kdr-cache")
            .join(PLANTUML_JAR_VERSION)
            .join("plantuml.jar")
    );
    assert_eq!(
        PlantUmlJarAssetOps::digest_bytes(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
}
