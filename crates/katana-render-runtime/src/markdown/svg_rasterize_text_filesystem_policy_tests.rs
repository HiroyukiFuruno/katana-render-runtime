use super::durable_generation;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);

#[test]
fn production_policy_allows_apfs_zero_fraction_and_keeps_linux_precision_guard() {
    assert!(durable_generation(true, 0, false));
    assert!(durable_generation(true, 1, false));
    assert!(!durable_generation(true, 0, true));
    assert!(durable_generation(true, 1, true));
    assert!(!durable_generation(false, 0, false));
    assert!(!durable_generation(false, 1, true));
}

#[test]
fn production_path_policy_rejects_stat_errors_and_respects_platform_precision_policy()
-> Result<(), Box<dyn std::error::Error>> {
    let missing_path = temporary_file_path();
    assert!(!super::durable_reusable(&missing_path, 1));

    let existing_path = temporary_file_path();
    std::fs::write(&existing_path, b"font stamp")?;
    #[cfg(target_os = "macos")]
    {
        let filesystem_supported = nix::sys::statfs::statfs(&existing_path)
            .is_ok_and(|stats| super::supported_filesystem_name(stats.filesystem_type_name()));
        assert_eq!(
            super::durable_reusable(&existing_path, 0),
            filesystem_supported
        );
    }
    #[cfg(target_os = "linux")]
    assert!(!super::durable_reusable(&existing_path, 0));
    std::fs::remove_file(existing_path)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn production_filesystem_classifier_accepts_only_apfs_names() {
    use super::supported_filesystem_name;

    assert!(supported_filesystem_name("apfs"));
    for unsupported in ["msdos", "exfat", "nfs", "overlay", "\0"] {
        assert!(!supported_filesystem_name(unsupported));
    }
}

#[cfg(target_os = "linux")]
#[test]
fn production_filesystem_classifier_accepts_only_allowlisted_linux_types() {
    use super::supported_filesystem_type;
    use nix::sys::statfs::{BTRFS_SUPER_MAGIC, EXT4_SUPER_MAGIC, FsType, TMPFS_MAGIC};

    assert!(supported_filesystem_type(EXT4_SUPER_MAGIC));
    assert!(supported_filesystem_type(BTRFS_SUPER_MAGIC));
    assert!(supported_filesystem_type(TMPFS_MAGIC));
    #[cfg(not(target_env = "musl"))]
    assert!(supported_filesystem_type(nix::sys::statfs::XFS_SUPER_MAGIC));
    for unsupported in [0, 0x6969, 0x0201_1ba1] {
        assert!(!supported_filesystem_type(FsType(unsupported)));
    }
}

#[cfg(target_os = "macos")]
#[test]
fn local_apfs_font_stamp_keeps_durable_reuse() -> Result<(), Box<dyn std::error::Error>> {
    let path = temporary_file_path();
    std::fs::write(&path, b"font stamp")?;
    let metadata = std::fs::metadata(&path)?;
    assert!(super::durable_reusable(&path, metadata.ctime_nsec()));
    std::fs::remove_file(path)?;
    Ok(())
}

#[cfg(target_os = "macos")]
#[test]
fn system_font_with_zero_ctime_fraction_remains_durably_reusable()
-> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new("/System/Library/Fonts/Menlo.ttc");
    let metadata = std::fs::metadata(path)?;
    assert_eq!(
        nix::sys::statfs::statfs(path)?.filesystem_type_name(),
        "apfs"
    );
    assert!(super::durable_reusable(path, metadata.ctime_nsec()));
    if metadata.ctime_nsec() == 0 {
        assert!(super::durable_reusable(path, 0));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
#[test]
fn local_linux_stamp_uses_the_production_filesystem_classifier()
-> Result<(), Box<dyn std::error::Error>> {
    let path = temporary_file_path();
    std::fs::write(&path, b"font stamp")?;
    let metadata = std::fs::metadata(&path)?;
    let filesystem_supported = nix::sys::statfs::statfs(&path)
        .map(|stats| super::supported_filesystem_type(stats.filesystem_type()))
        .unwrap_or(false);
    assert_eq!(
        super::durable_reusable(&path, metadata.ctime_nsec()),
        durable_generation(filesystem_supported, metadata.ctime_nsec(), true)
    );
    std::fs::remove_file(path)?;
    Ok(())
}

fn temporary_file_path() -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "krr-filesystem-policy-{}-{}.tmp",
        std::process::id(),
        NEXT_FILE.fetch_add(1, Ordering::Relaxed),
    ))
}
