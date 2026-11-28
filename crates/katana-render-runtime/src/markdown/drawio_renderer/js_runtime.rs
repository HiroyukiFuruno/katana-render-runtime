use super::js_runtime_resources::{DrawioResource, DrawioResourceCatalog};
use super::js_runtime_scripts::DrawioRuntimeScripts;
use crate::markdown::color_preset::DiagramColorPreset;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;
#[cfg(test)]
use crate::markdown::runtime_resource_loader::{
    BundledResource, RuntimeResourceLoader as runtime_resource_loader,
};
#[cfg(test)]
use std::path::Path;

pub(super) struct DrawioJsRuntimeOps;

impl DrawioJsRuntimeOps {
    #[cfg(test)]
    pub(super) fn render(
        source: &str,
        drawio_js: &Path,
        preset: &DiagramColorPreset,
    ) -> Result<String, String> {
        Self::render_with_origin(source, drawio_js, preset, false)
    }

    #[cfg(test)]
    pub(super) fn render_with_origin(
        source: &str,
        drawio_js: &Path,
        preset: &DiagramColorPreset,
        bundled: bool,
    ) -> Result<String, String> {
        let bundle =
            runtime_resource_loader::read(drawio_js, bundled.then_some(BundledResource::Drawio))
                .map_err(|error| format!("Failed to read Draw.io JavaScript bundle: {error}"))?;
        Self::render_bundle(source, &bundle, preset)
    }

    pub(super) fn render_bundle(
        source: &str,
        bundle: &str,
        preset: &DiagramColorPreset,
    ) -> Result<String, String> {
        let request = DrawioRenderRequest::new(source, preset)?;
        let request_json = request.to_json_value().to_string();
        let scripts = DrawioRuntimeScripts::build(bundle, &request_json);
        let svg = DiagramV8Runtime::render(&scripts)?;
        rendered_svg(svg)
    }
}

struct DrawioRenderRequest<'a> {
    source: &'a str,
    dark_mode: bool,
    background: &'a str,
    resources: Vec<DrawioResource>,
}

impl<'a> DrawioRenderRequest<'a> {
    fn new(source: &'a str, preset: &'a DiagramColorPreset) -> Result<Self, String> {
        Ok(Self {
            source,
            dark_mode: preset.dark_mode,
            background: preset.background.as_ref(),
            resources: DrawioResourceCatalog::builtin(source)?,
        })
    }

    fn to_json_value(&self) -> serde_json::Value {
        let resources = self.resources.iter().map(resource_json).collect::<Vec<_>>();
        serde_json::json!({
            "source": self.source,
            "dark_mode": self.dark_mode,
            "background": self.background,
            "resources": resources,
        })
    }
}

fn resource_json(resource: &DrawioResource) -> serde_json::Value {
    serde_json::json!({
        "path": resource.path,
        "mime_type": resource.mime_type,
        "content": resource.content,
        "encoding": resource.encoding.as_str(),
    })
}

#[cfg(test)]
fn read_drawio_bundle(drawio_js: &Path) -> Result<String, String> {
    runtime_resource_loader::read_file(drawio_js)
        .map_err(|error| format!("Failed to read Draw.io JavaScript bundle: {error}"))
}

fn ensure_svg(svg: &str) -> Result<(), String> {
    if svg.contains("<svg") && svg.contains("</svg>") {
        return Ok(());
    }
    Err("Draw.io JavaScript did not return SVG markup".to_string())
}

fn rendered_svg(svg: String) -> Result<String, String> {
    match ensure_svg(&svg) {
        Ok(()) => Ok(svg),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
#[path = "js_runtime_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "js_runtime_stencil_test_support.rs"]
mod stencil_test_support;

#[cfg(test)]
#[path = "js_runtime_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "js_runtime_html_dom_tests.rs"]
mod html_dom_tests;

#[cfg(test)]
#[path = "js_runtime_shadow_tests.rs"]
mod shadow_tests;

#[cfg(test)]
#[path = "js_runtime_color_tests.rs"]
mod color_tests;

#[cfg(test)]
#[path = "js_runtime_shape_color_tests.rs"]
mod shape_color_tests;

#[cfg(test)]
#[path = "js_runtime_neutral_color_tests.rs"]
mod neutral_color_tests;

#[cfg(test)]
#[path = "js_runtime_path_color_tests.rs"]
mod path_color_tests;

#[cfg(test)]
#[path = "js_runtime_palette_color_tests.rs"]
mod palette_color_tests;

#[cfg(test)]
#[path = "js_runtime_floorplan_color_tests.rs"]
mod floorplan_color_tests;

#[cfg(test)]
#[path = "js_runtime_table_color_tests.rs"]
mod table_color_tests;

#[cfg(test)]
#[path = "js_runtime_page_bounds_tests.rs"]
mod page_bounds_tests;

#[cfg(test)]
#[path = "js_runtime_page_crop_test_support.rs"]
mod page_crop_test_support;

#[cfg(test)]
#[path = "js_runtime_page_crop_tests.rs"]
mod page_crop_tests;

#[cfg(test)]
#[path = "js_runtime_device_page_test_support.rs"]
mod device_page_test_support;

#[cfg(test)]
#[path = "js_runtime_device_page_crop_tests.rs"]
mod device_page_crop_tests;

#[cfg(test)]
#[path = "js_runtime_device_page_export_tests.rs"]
mod device_page_export_tests;

#[cfg(test)]
#[path = "js_runtime_page_layout_crop_tests.rs"]
mod page_layout_crop_tests;

#[cfg(test)]
#[path = "js_runtime_export_top_tests.rs"]
mod export_top_tests;

#[cfg(test)]
#[path = "js_runtime_label_crop_tests.rs"]
mod label_crop_tests;

#[cfg(test)]
#[path = "js_runtime_html_label_tests.rs"]
mod html_label_tests;

#[cfg(test)]
#[path = "js_runtime_html_label_content_tests.rs"]
mod html_label_content_tests;

#[cfg(test)]
#[path = "js_runtime_edge_label_tests.rs"]
mod edge_label_tests;

#[cfg(test)]
#[path = "js_runtime_geometry_tests.rs"]
mod geometry_tests;

#[cfg(test)]
#[path = "js_runtime_html_text_source_color_tests.rs"]
mod html_text_source_color_tests;

#[cfg(test)]
#[path = "js_runtime_html_text_explicit_color_tests.rs"]
mod html_text_explicit_color_tests;

#[cfg(test)]
#[path = "js_runtime_image_label_tests.rs"]
mod image_label_tests;

#[cfg(test)]
#[path = "js_runtime_plain_text_label_tests.rs"]
mod plain_text_label_tests;
