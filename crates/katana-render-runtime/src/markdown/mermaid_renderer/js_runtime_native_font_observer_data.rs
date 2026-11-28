#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(super) struct FontObservationEnvelope {
    pub(super) svg: String,
    pub(super) x_height_observations: Vec<XHeightObservation>,
    pub(super) svg_metric_observations: Vec<SvgMetricObservation>,
    pub(super) bbox_observations: Vec<SvgBBoxObservation>,
    pub(super) inner_html_observations: Vec<InnerHtmlObservation>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FontObservations<'a> {
    pub(super) x_height: &'a [XHeightObservation],
    pub(super) svg_metrics: &'a [SvgMetricObservation],
    pub(super) bbox: &'a [SvgBBoxObservation],
    pub(super) inner_html: &'a [InnerHtmlObservation],
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct XHeightObservation {
    family: String,
    size: Option<f64>,
    weight: Option<f64>,
    italic: bool,
    x_height: Option<f64>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SvgMetricObservation {
    text: String,
    family: String,
    size: Option<f64>,
    weight: Option<f64>,
    italic: bool,
    advance: Option<f64>,
    ascent: Option<f64>,
    descent: Option<f64>,
    #[serde(rename = "inkLeft")]
    ink_left: Option<f64>,
    ink_top: Option<f64>,
    #[serde(rename = "inkRight")]
    ink_right: Option<f64>,
    #[serde(rename = "inkBottom")]
    ink_bottom: Option<f64>,
    layout_ascent: Option<f64>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SvgBBoxObservation {
    local_name: String,
    class_name: String,
    id: String,
    text: String,
    transform_attribute: String,
    style_attribute: String,
    display_attribute: String,
    visibility_attribute: String,
    inline_display: String,
    inline_visibility: String,
    x: Option<f64>,
    y: Option<f64>,
    width: Option<f64>,
    height: Option<f64>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InnerHtmlObservation {
    target_local_name: String,
    input_prefix: String,
    input_length: usize,
    input_has_svg: bool,
    output_children: Vec<InnerHtmlChildObservation>,
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct InnerHtmlChildObservation {
    local_name: String,
    #[serde(rename = "namespaceURI")]
    namespace_uri: String,
    child_count: usize,
}

impl FontObservationEnvelope {
    pub(super) fn observations(&self) -> FontObservations<'_> {
        FontObservations {
            x_height: &self.x_height_observations,
            svg_metrics: &self.svg_metric_observations,
            bbox: &self.bbox_observations,
            inner_html: &self.inner_html_observations,
        }
    }
}

pub(super) fn decode_observations(envelope: &str) -> Result<(String, String), String> {
    serde_json::from_str::<FontObservationEnvelope>(envelope)
        .and_then(|observed| {
            let diagnostic = serde_json::to_string(&observed.observations())?;
            Ok((observed.svg, diagnostic))
        })
        .map_err(|error| error.to_string())
}
