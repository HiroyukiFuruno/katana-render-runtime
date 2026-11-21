use super::{FONT_BYTES, check_nondurable_file_pair, file_database, font_source_generation};
use crate::system::ProcessService;
use std::io;
use std::path::{Path, PathBuf};

const IMAGE_SIZE: &str = "24m";

struct MountedFontVolume {
    directory: tempfile::TempDir,
    mounted: bool,
}

impl MountedFontVolume {
    fn new() -> io::Result<Self> {
        let directory = tempfile::tempdir()?;
        let image = directory.path().join("font-cache.dmg");
        let mut volume = Self {
            directory,
            mounted: false,
        };
        std::fs::create_dir(volume.path())?;
        hdiutil(["create", "-size", IMAGE_SIZE, "-fs", "HFS+"], [&image])?;
        hdiutil(
            ["attach", "-nobrowse", "-mountpoint"],
            [&volume.path(), &image],
        )?;
        volume.mounted = true;
        Ok(volume)
    }

    fn path(&self) -> PathBuf {
        self.directory.path().join("volume")
    }

    fn detach(&mut self) -> io::Result<()> {
        if self.mounted {
            hdiutil(["detach"], [&self.path()])?;
            self.mounted = false;
        }
        Ok(())
    }
}

impl Drop for MountedFontVolume {
    fn drop(&mut self) {
        if self.detach().is_err() {
            self.directory.disable_cleanup(true);
        }
    }
}

fn hdiutil<'a>(
    flags: impl IntoIterator<Item = &'a str>,
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> io::Result<()> {
    let output = ProcessService::create_command("/usr/bin/hdiutil")
        .args(flags)
        .args(paths)
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(io::Error::other(String::from_utf8_lossy(&output.stderr)))
}

fn font_on_volume(path: &Path) -> io::Result<(super::Database, resvg::usvg::fontdb::ID)> {
    std::fs::write(path, FONT_BYTES)?;
    let (database, face) = file_database(path).map_err(io::Error::other)?;
    let generation = font_source_generation(&database, face);
    assert!(generation.reusable() && !generation.durable_reusable());
    Ok((database, face))
}

#[test]
fn nondurable_font_is_usable_without_retaining_any_cached_handle()
-> Result<(), Box<dyn std::error::Error>> {
    let mut volume = MountedFontVolume::new()?;
    let (first, first_id) = font_on_volume(&volume.path().join("first.ttf"))?;
    let (second, second_id) = font_on_volume(&volume.path().join("second.ttf"))?;
    check_nondurable_file_pair(&first, first_id, &second, second_id)?;
    volume.detach()?;
    Ok(())
}
