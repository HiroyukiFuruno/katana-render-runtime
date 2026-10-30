//! SVGの依存実装を同じ配布物へ収容する内部境界。

#[rustfmt::skip]
#[path = "../vendor/svg-engine/resvg/lib.rs"]
pub(crate) mod resvg;

#[rustfmt::skip]
#[path = "../vendor/svg-engine/usvg/lib.rs"]
pub(crate) mod usvg;

#[cfg(test)]
#[path = "svg_engine_writer_tests.rs"]
mod writer_tests;

#[cfg(test)]
#[path = "svg_engine_parser_boundary_tests.rs"]
mod parser_boundary_tests;

#[cfg(test)]
#[path = "svg_engine_render_tests.rs"]
mod render_tests;

#[cfg(test)]
#[path = "svg_engine_viewbox_tests.rs"]
mod viewbox_tests;
