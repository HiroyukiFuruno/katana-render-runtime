use super::api::RenderError;

const VIEW_BOX_COMPONENT_COUNT: usize = 4;
const VIEW_BOX_WIDTH_INDEX: usize = 2;
const VIEW_BOX_HEIGHT_INDEX: usize = 3;

pub(super) struct SvgMetadata {
    pub(super) width: f32,
    pub(super) height: f32,
    pub(super) view_box: String,
}

pub(super) struct SvgMetadataOps;

impl SvgMetadataOps {
    pub(super) fn parse(svg: &str) -> Result<SvgMetadata, RenderError> {
        if svg.trim().is_empty() {
            return Ok(Self::empty());
        }
        let root = xmltree::Element::parse(svg.as_bytes())
            .map_err(|error| RenderError::Runtime(error.to_string()))?;
        let view_box = Self::view_box(&root);
        let (width, height) = Self::dimensions(&root, &view_box)?;
        Ok(SvgMetadata {
            width,
            height,
            view_box,
        })
    }

    fn empty() -> SvgMetadata {
        SvgMetadata {
            width: 0.0,
            height: 0.0,
            view_box: String::new(),
        }
    }

    fn view_box(root: &xmltree::Element) -> String {
        match root
            .attributes
            .get("viewBox")
            .or_else(|| root.attributes.get("viewbox"))
        {
            Some(value) => value.clone(),
            None => String::new(),
        }
    }

    fn dimensions(root: &xmltree::Element, view_box: &str) -> Result<(f32, f32), RenderError> {
        let width = root
            .attributes
            .get("width")
            .and_then(|value| Self::number(value));
        let height = root
            .attributes
            .get("height")
            .and_then(|value| Self::number(value));
        match (width, height) {
            (Some(width), Some(height)) => Ok((width, height)),
            _ => Self::view_box_dimensions(view_box),
        }
    }

    fn number(value: &str) -> Option<f32> {
        value.trim().trim_end_matches("px").parse::<f32>().ok()
    }

    fn view_box_dimensions(view_box: &str) -> Result<(f32, f32), RenderError> {
        let numbers = view_box
            .split_whitespace()
            .filter_map(|part| part.parse::<f32>().ok())
            .collect::<Vec<_>>();
        if numbers.len() == VIEW_BOX_COMPONENT_COUNT {
            return Ok((
                numbers[VIEW_BOX_WIDTH_INDEX],
                numbers[VIEW_BOX_HEIGHT_INDEX],
            ));
        }
        Err(RenderError::InvalidInput(
            "SVG size metadata is missing".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::SvgMetadataOps;

    #[test]
    fn parse_uses_view_box_when_width_height_are_missing() -> Result<(), Box<dyn std::error::Error>>
    {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 10"></svg>"#;
        let metadata = SvgMetadataOps::parse(svg)?;
        assert_eq!(metadata.width, 20.0);
        assert_eq!(metadata.height, 10.0);
        Ok(())
    }

    #[test]
    fn parse_handles_empty_svg_and_rejects_missing_size() {
        let empty = SvgMetadataOps::parse(" ");
        assert!(empty.as_ref().is_ok_and(|it| it.width == 0.0));
        assert!(empty.as_ref().is_ok_and(|it| it.view_box.is_empty()));

        let missing = SvgMetadataOps::parse(r#"<svg xmlns="http://www.w3.org/2000/svg"></svg>"#);
        assert!(missing.is_err());

        let invalid_xml = SvgMetadataOps::parse("<svg>");
        assert!(invalid_xml.is_err());
    }
}
