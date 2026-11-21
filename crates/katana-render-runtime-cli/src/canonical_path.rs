use std::path::{Path, PathBuf};

pub(crate) struct CanonicalPath;

impl CanonicalPath {
    pub(crate) fn for_comparison(path: &Path) -> PathBuf {
        #[cfg(windows)]
        {
            return Self::normalize(path);
        }
        #[cfg(not(windows))]
        path.to_path_buf()
    }
}

#[cfg(windows)]
impl CanonicalPath {
    fn normalize(path: &Path) -> PathBuf {
        let mut components = path.components();
        let Some(std::path::Component::Prefix(prefix)) = components.next() else {
            return path.to_path_buf();
        };
        let Some(mut normalized) = Self::verbatim_prefix_path(prefix.kind()) else {
            return path.to_path_buf();
        };
        for component in
            components.filter(|component| !matches!(component, std::path::Component::RootDir))
        {
            normalized.push(component.as_os_str());
        }
        normalized
    }

    fn verbatim_prefix_path(prefix: std::path::Prefix<'_>) -> Option<PathBuf> {
        match prefix {
            std::path::Prefix::VerbatimDisk(drive) => {
                Some(PathBuf::from(format!("{}:\\", char::from(drive))))
            }
            std::path::Prefix::VerbatimUNC(server, share) => Some(Self::unc_path(server, share)),
            _ => None,
        }
    }

    fn unc_path(server: &std::ffi::OsStr, share: &std::ffi::OsStr) -> PathBuf {
        let mut unc = std::ffi::OsString::from(r"\\");
        unc.push(server);
        unc.push("\\");
        unc.push(share);
        unc.push("\\");
        PathBuf::from(unc)
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::CanonicalPath;
    use std::path::{Path, PathBuf};

    #[test]
    fn canonical_comparison_converts_verbatim_disk_and_unc_prefixes_only() {
        assert_eq!(
            CanonicalPath::for_comparison(Path::new(r"\\?\C:\work\input.mmd")),
            PathBuf::from(r"C:\work\input.mmd")
        );
        assert_eq!(
            CanonicalPath::for_comparison(Path::new(r"\\?\UNC\server\share\input.mmd")),
            PathBuf::from(r"\\server\share\input.mmd")
        );
        assert_ne!(
            CanonicalPath::for_comparison(Path::new(r"\\?\C:\work\input.mmd")),
            CanonicalPath::for_comparison(Path::new(r"C:\work\child\..\input.mmd"))
        );
    }
}
