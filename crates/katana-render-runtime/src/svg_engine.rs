//! SVGの依存実装を同じ配布物へ収容する内部境界。

#[rustfmt::skip]
#[path = "../vendor/svg-engine/resvg/lib.rs"]
pub(crate) mod resvg;

#[rustfmt::skip]
#[path = "../vendor/svg-engine/usvg/lib.rs"]
pub(crate) mod usvg;
