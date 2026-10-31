//! SVGの依存実装を同じ配布物へ収容する内部境界。

#[cfg(test)]
/* WHY: 外部vendorのallow継承を打ち消し、KRR所有SVGモデルテストへ厳格lintを適用する。 */
macro_rules! owned_svg_model_tests {
    ($path:literal) => {
        #[deny(
            dead_code,
            unused_imports,
            clippy::all,
            clippy::cognitive_complexity,
            clippy::too_many_lines,
            clippy::unwrap_used,
            clippy::wildcard_imports
        )]
        #[path = $path]
        mod model_tests;
    };
}

#[cfg(test)]
pub(crate) use owned_svg_model_tests;

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
#[path = "svg_engine_parser_style_boundary_tests.rs"]
mod parser_style_boundary_tests;

#[cfg(test)]
#[path = "svg_engine_use_boundary_coverage_tests.rs"]
mod use_boundary_coverage_tests;

#[cfg(test)]
#[path = "svg_engine_render_tests.rs"]
mod render_tests;

#[cfg(test)]
#[path = "svg_engine_viewbox_tests.rs"]
mod viewbox_tests;
