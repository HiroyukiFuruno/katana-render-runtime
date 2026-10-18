use super::Mode;
use crate::markdown::svg_rasterize::text_shaping;
use resvg::usvg::fontdb::{FaceInfo, Source};
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
};
pub(super) type ByteWeak = Weak<dyn AsRef<[u8]> + Send + Sync>;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub(in super::super) struct FaceSource {
    pub(in super::super) kind: String,
    pub(in super::super) path: Option<PathBuf>,
    pub(in super::super) stamp: Option<String>,
    pub(in super::super) bytes_sha256: Option<String>,
    pub(in super::super) index: u32,
    pub(in super::super) families: Vec<String>,
    pub(in super::super) post_script_name: String,
    pub(in super::super) style: String,
    pub(in super::super) weight: u16,
    pub(in super::super) stretch: String,
    pub(in super::super) monospaced: bool,
}
pub(super) fn source_record(face: &FaceInfo, mode: Mode) -> Option<(FaceSource, Option<ByteWeak>)> {
    let (kind, path, weak) = source_metadata(&face.source);
    let (stamp, bytes_sha256) = if matches!(mode, Mode::Audit) {
        audited_source(face)?
    } else {
        (None, None)
    };
    Some((
        FaceSource {
            kind,
            path,
            stamp,
            bytes_sha256,
            index: face.index,
            families: face.families.iter().map(|(name, _)| name.clone()).collect(),
            post_script_name: face.post_script_name.clone(),
            style: format!("{:?}", face.style),
            weight: face.weight.0,
            stretch: format!("{:?}", face.stretch),
            monospaced: face.monospaced,
        },
        weak,
    ))
}
fn source_metadata(source: &Source) -> (String, Option<PathBuf>, Option<ByteWeak>) {
    match source {
        Source::File(path) => ("File".into(), Some(path.clone()), None),
        Source::SharedFile(path, bytes) => (
            "SharedFile".into(),
            Some(path.clone()),
            Some(Arc::downgrade(bytes)),
        ),
        Source::Binary(_) => ("Binary".into(), None, None),
    }
}
fn audited_source(face: &FaceInfo) -> Option<(Option<String>, Option<String>)> {
    use sha2::{Digest, Sha256};
    let path = match &face.source {
        Source::File(path) | Source::SharedFile(path, _) => path,
        Source::Binary(_) => return None,
    };
    let before = text_shaping::stamp_path_uncached(path).ok()?;
    let (_, bytes) = source_bytes(&face.source)?;
    let sha = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let after = text_shaping::stamp_path_uncached(path).ok()?;
    (before == after).then(|| (Some(format!("{before:?}")), Some(sha)))
}
fn source_bytes(source: &Source) -> Option<(Option<PathBuf>, Vec<u8>)> {
    match source {
        Source::File(path) => Some((Some(path.clone()), std::fs::read(path).ok()?)),
        Source::SharedFile(path, data) => {
            Some((Some(path.clone()), data.as_ref().as_ref().to_vec()))
        }
        Source::Binary(data) => Some((None, data.as_ref().as_ref().to_vec())),
    }
}
