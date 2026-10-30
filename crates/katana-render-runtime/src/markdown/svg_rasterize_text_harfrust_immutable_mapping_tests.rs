use super::{eligible_mount, eligible_source};
use nix::mount::MntFlags;
use resvg::usvg::fontdb::{Database, Source};
use std::io::Write;

const ALL_FLAGS: MntFlags = MntFlags::MNT_RDONLY
    .union(MntFlags::MNT_ROOTFS)
    .union(MntFlags::MNT_SNAPSHOT);

#[test]
fn immutable_mapping_requires_apfs_all_flags_and_root_device() {
    assert!(eligible_mount("apfs", ALL_FLAGS, true));
    assert!(!eligible_mount("apfs", ALL_FLAGS, false));
    for missing in [
        MntFlags::MNT_RDONLY,
        MntFlags::MNT_ROOTFS,
        MntFlags::MNT_SNAPSHOT,
    ] {
        assert!(!eligible_mount("apfs", ALL_FLAGS.difference(missing), true));
    }
    assert!(!eligible_mount("hfs", ALL_FLAGS, true));
}

#[test]
fn ordinary_mutable_temp_file_is_not_eligible() -> Result<(), String> {
    let mut temp = tempfile::NamedTempFile::new().map_err(|error| error.to_string())?;
    temp.write_all(include_bytes!("../../assets/fonts/NotoSans-Regular.ttf"))
        .map_err(|error| error.to_string())?;
    let path = temp.path().to_path_buf();
    let mut database = Database::new();
    database.load_font_source(Source::File(path.clone()));
    let face = database
        .faces()
        .next()
        .ok_or("temporary font face missing")?
        .id;
    let length = std::fs::metadata(&path)
        .map_err(|error| error.to_string())?
        .len();
    let result = eligible_source(&database, face, &path, 0, length);
    assert!(!result?);
    let mut permissions = temp
        .as_file()
        .metadata()
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_readonly(true);
    temp.as_file()
        .set_permissions(permissions)
        .map_err(|error| error.to_string())?;
    assert!(!eligible_source(&database, face, &path, 0, length)?);
    Ok(())
}
