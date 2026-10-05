fn durable_generation(
    filesystem_supported: bool,
    changed_nanoseconds: i64,
    require_nonzero_ctime_fraction: bool,
) -> bool {
    filesystem_supported && (!require_nonzero_ctime_fraction || changed_nanoseconds != 0)
}

#[cfg(target_os = "macos")]
fn supported_filesystem_name(name: &str) -> bool {
    name == "apfs"
}

#[cfg(target_os = "linux")]
fn supported_filesystem_type(filesystem_type: nix::sys::statfs::FsType) -> bool {
    use nix::sys::statfs::{BTRFS_SUPER_MAGIC, EXT4_SUPER_MAGIC, TMPFS_MAGIC};

    let allowlisted = filesystem_type == EXT4_SUPER_MAGIC
        || filesystem_type == BTRFS_SUPER_MAGIC
        || filesystem_type == TMPFS_MAGIC;
    #[cfg(not(target_env = "musl"))]
    {
        allowlisted || filesystem_type == nix::sys::statfs::XFS_SUPER_MAGIC
    }
    #[cfg(target_env = "musl")]
    {
        allowlisted
    }
}

pub(super) fn durable_reusable(path: &std::path::Path, changed_nanoseconds: i64) -> bool {
    #[cfg(target_os = "linux")]
    if changed_nanoseconds == 0 {
        return false;
    }
    #[cfg(target_os = "macos")]
    {
        nix::sys::statfs::statfs(path).ok().is_some_and(|stats| {
            /* WHY: ファイルのctime小数部が0でも、APFS自体の精度を否定する根拠にはならない。 */
            durable_generation(
                supported_filesystem_name(stats.filesystem_type_name()),
                changed_nanoseconds,
                false,
            )
        })
    }
    #[cfg(target_os = "linux")]
    {
        nix::sys::statfs::statfs(path).ok().is_some_and(|stats| {
            durable_generation(
                supported_filesystem_type(stats.filesystem_type()),
                changed_nanoseconds,
                true,
            )
        })
    }
}

#[cfg(test)]
#[path = "svg_rasterize_text_filesystem_policy_tests.rs"]
mod tests;
