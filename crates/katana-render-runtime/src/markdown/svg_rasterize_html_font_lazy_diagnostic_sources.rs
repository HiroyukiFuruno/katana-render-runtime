use super::Mode;
use crate::markdown::svg_rasterize::text_shaping;
use resvg::usvg::fontdb::{FaceInfo, Source};
use std::{
    borrow::Cow,
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
    let (stamp, bytes_sha256) = if matches!(mode, Mode::Audit) {
        audited_source(face)?
    } else {
        (None, None)
    };
    let (mut record, owner) = measured_source_record(face);
    record.stamp = stamp;
    record.bytes_sha256 = bytes_sha256;
    Some((record, owner))
}

pub(super) fn measured_source_record(face: &FaceInfo) -> (FaceSource, Option<ByteWeak>) {
    let (kind, path, owner) = source_metadata(&face.source);
    /* 測定記録は全Sourceのメタデータから構成でき、物理検証だけが失敗し得る。 */
    (
        FaceSource {
            kind,
            path,
            stamp: None,
            bytes_sha256: None,
            index: face.index,
            families: face.families.iter().map(|(name, _)| name.clone()).collect(),
            post_script_name: face.post_script_name.clone(),
            style: format!("{:?}", face.style),
            weight: face.weight.0,
            stretch: format!("{:?}", face.stretch),
            monospaced: face.monospaced,
        },
        owner,
    )
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
    let (path, shared_bytes) = match &face.source {
        Source::File(path) => (path, None),
        Source::SharedFile(path, data) => (path, Some(data.as_ref().as_ref())),
        Source::Binary(_) => return None,
    };
    let before = text_shaping::stamp_path_uncached(path).ok()?;
    let bytes = match shared_bytes {
        Some(bytes) => Cow::Borrowed(bytes),
        None => Cow::Owned(std::fs::read(path).ok()?),
    };
    let sha = Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let after = text_shaping::stamp_path_uncached(path).ok()?;
    (before == after).then(|| (Some(format!("{before:?}")), Some(sha)))
}
