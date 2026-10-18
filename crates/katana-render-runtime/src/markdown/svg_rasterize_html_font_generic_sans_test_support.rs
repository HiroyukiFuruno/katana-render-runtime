#[path = "svg_rasterize_html_font_generic_sans_aggregate.rs"]
mod aggregate;
#[path = "svg_rasterize_html_font_generic_sans_empty_admission.rs"]
mod empty_admission;
#[path = "svg_rasterize_html_font_generic_sans_empty_measurement.rs"]
mod empty_measurement;
#[path = "svg_rasterize_html_font_generic_sans_face_assertions.rs"]
mod face_assertions;
#[path = "svg_rasterize_html_font_generic_sans_interaction.rs"]
mod interaction;

pub(super) use face_assertions::{
    admitted, assert_same_face, eager_render, known_database, metrics,
};

pub(super) use empty_admission::{assert_empty_admission_rejections, assert_empty_style};

pub(super) use empty_measurement::{assert_empty_measurement_pair, assert_nonempty_latin_metrics};

pub(super) use interaction::{
    check_initial_generic_fixture, initial_frame_sha, interaction_session, print_font_decisions,
    print_initial_frame, record_raster_phase,
};

pub(super) use aggregate::check_aggregate_faces_and_metrics;
