use crate::markdown::MarkdownError;

pub(crate) struct NativeSvgRoot;

impl NativeSvgRoot {
    pub(crate) fn with_numeric_size(
        svg: &str,
        width: u32,
        height: u32,
    ) -> Result<String, MarkdownError> {
        let Some(open_end) = svg.find('>') else {
            return Ok(svg.to_string());
        };
        let open_tag = &svg[..open_end];
        let rest = &svg[open_end..];
        let sized_open_tag = Self::set_attribute(open_tag, "width", &width.to_string())?;
        let sized_open_tag = Self::set_attribute(&sized_open_tag, "height", &height.to_string())?;
        Ok(format!("{sized_open_tag}{rest}"))
    }

    fn set_attribute(open_tag: &str, name: &str, value: &str) -> Result<String, MarkdownError> {
        let Some(start) = Self::attribute_start(open_tag, name) else {
            return Ok(format!(r#"{open_tag} {name}="{value}""#));
        };
        let value_start = start + format!(r#" {name}=""#).len();
        let Some(end) = open_tag[value_start..].find('"') else {
            return Err(MarkdownError::ExportFailed(format!(
                "SVG attribute `{name}` is missing a closing quote"
            )));
        };
        Ok(format!(
            r#"{} {name}="{value}"{}"#,
            &open_tag[..start],
            &open_tag[value_start + end + 1..]
        ))
    }

    fn attribute_start(open_tag: &str, name: &str) -> Option<usize> {
        let marker = format!(r#" {name}=""#);
        if let Some(start) = open_tag.find(&marker) {
            return Some(start);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::NativeSvgRoot;

    #[test]
    fn rewrites_existing_svg_dimensions() -> Result<(), Box<dyn std::error::Error>> {
        let svg = r#"<svg width="10" height="20"><rect /></svg>"#;
        let sized = NativeSvgRoot::with_numeric_size(svg, 320, 240)?;

        assert!(sized.starts_with(r#"<svg width="320" height="240">"#));
        Ok(())
    }

    #[test]
    fn appends_missing_svg_dimensions() -> Result<(), Box<dyn std::error::Error>> {
        let svg = r#"<svg viewBox="0 0 10 20"><rect /></svg>"#;
        let sized = NativeSvgRoot::with_numeric_size(svg, 10, 20)?;

        assert!(sized.starts_with(r#"<svg viewBox="0 0 10 20" width="10" height="20">"#));
        Ok(())
    }

    #[test]
    fn leaves_text_without_svg_open_tag_unchanged() {
        let sized = NativeSvgRoot::with_numeric_size("plain", 10, 20);
        assert!(sized.as_ref().is_ok_and(|it| it == "plain"));
    }

    #[test]
    fn rejects_malformed_existing_dimension_attribute() {
        let sized = NativeSvgRoot::with_numeric_size(r#"<svg width="10><rect /></svg>"#, 10, 20);

        assert!(sized.is_err());
    }
}
