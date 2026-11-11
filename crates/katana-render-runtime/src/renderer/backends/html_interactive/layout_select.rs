use super::super::html_document::HtmlDocumentNode;
use super::constants::{
    DEFAULT_SELECT_WIDTH, SELECT_ARROW_WIDTH, SELECT_HEIGHT, SELECT_TEXT_PADDING,
};
use super::control_style::button_style;
use super::document::{attribute, node_text};
use super::layout::HtmlLayoutRenderer;
use super::style::CssStyle;
use super::text_metrics::text_width;
use super::types::{ElementRenderContext, HitTargetKind, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn render_select(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let x = layout.x + layout.style.margin_left;
        let y = layout.y + layout.style.margin_top;
        let options = option_texts(element.children);
        let selected = selected_option_text(&options);
        let author_border_width = layout.style.border_width;
        let mut style = button_style(layout.style);
        if author_border_width == 0.0 {
            style.border_width = 1.0;
        }
        let width = select_width(&options, layout.width, &style);
        let height = select_height(&style);
        self.paint_box(x, y, width, height, &style);
        self.paint_select_value(&selected, x, y, width, height, &style);
        self.push_target(element.node_id, x, y, width, height, HitTargetKind::Click);
        y + height + style.margin_bottom
    }

    fn paint_select_value(
        &mut self,
        selected: &str,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        style: &CssStyle,
    ) {
        let content_x = x + style.border_left_width() + style.padding_left;
        self.paint_control_text(
            selected,
            content_x + SELECT_TEXT_PADDING,
            (style.content_width(width) - SELECT_ARROW_WIDTH).max(0.0),
            y,
            height,
            style,
        );
        let arrow_x = x + width - SELECT_ARROW_WIDTH / 2.0;
        let arrow_y = y + height / 2.0;
        self.svg.push_str(&format!(
            r##"<path d="M {} {} l 4 0 l -2 3 z" fill="#4b5563"/>"##,
            arrow_x - 2.0,
            arrow_y - 1.5,
        ));
    }
}

fn select_width(options: &[(String, bool)], available: f32, style: &CssStyle) -> f32 {
    if let Some(width) = style.explicit_width(available) {
        return width;
    }
    let intrinsic = options
        .iter()
        .map(|(option, _)| text_width(option, style))
        .fold(0.0, f32::max)
        + SELECT_ARROW_WIDTH
        + SELECT_TEXT_PADDING * 2.0;
    intrinsic.max(DEFAULT_SELECT_WIDTH)
}

fn select_height(style: &CssStyle) -> f32 {
    style
        .height
        .map(|height| style.outer_height(height))
        .unwrap_or(SELECT_HEIGHT)
        .max(style.minimum_outer_height())
}

fn option_texts(children: &[HtmlDocumentNode]) -> Vec<(String, bool)> {
    children.iter().flat_map(option_texts_node).collect()
}

fn option_texts_node(node: &HtmlDocumentNode) -> Vec<(String, bool)> {
    match node {
        HtmlDocumentNode::Element {
            tag,
            children,
            attributes,
            ..
        } if tag == "option" => vec![(
            node_text(children),
            attribute(attributes, "selected").is_some(),
        )],
        HtmlDocumentNode::Element { children, .. } => {
            children.iter().flat_map(option_texts_node).collect()
        }
        HtmlDocumentNode::Text(_) => Vec::new(),
    }
}

fn selected_option_text(options: &[(String, bool)]) -> String {
    match options
        .iter()
        .find(|(_, selected)| *selected)
        .or_else(|| options.first())
    {
        Some((text, _)) => text.clone(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::super::HtmlInteractiveSession;
    use super::super::tests::test_error;
    use super::{SELECT_HEIGHT, option_texts, selected_option_text};
    use crate::renderer::backends::html_browser::{HtmlBrowserSource, HtmlBrowserViewport};
    use crate::renderer::backends::html_document::HtmlDocumentNode;

    #[test]
    fn option_texts_walks_nested_elements_and_selected_option() {
        let options = vec![HtmlDocumentNode::Element {
            tag: "div".to_string(),
            attributes: Vec::new(),
            children: vec![HtmlDocumentNode::Element {
                node_id: 2,
                tag: "option".to_string(),
                attributes: vec![("selected".to_string(), String::new())],
                children: vec![HtmlDocumentNode::Text("Nested choice".to_string())],
            }],
            node_id: 1,
        }];

        let choices = option_texts(&options);
        assert_eq!(choices, vec![("Nested choice".to_string(), true)]);
        assert_eq!(selected_option_text(&choices), "Nested choice");
    }

    #[test]
    fn selected_option_falls_back_to_first_and_empty() {
        let choices = vec![("First".to_string(), false), ("Second".to_string(), false)];
        assert_eq!(selected_option_text(&choices), "First");
        assert_eq!(selected_option_text(&[]), "");
    }

    fn parsed_select_session() -> Result<HtmlInteractiveSession, String> {
        let source = HtmlBrowserSource::new(
            r#"<style>html,body{margin:0}</style>
<select id=explicit style="box-sizing:border-box;width:180px;height:44px">
  <optgroup label=Group><option>fallback</option><option selected>chosen</option></optgroup>
</select>
<select id=default><option>fallback</option><option>other</option></select>
<select id=empty style="box-sizing:border-box;width:80px"></select>"#,
            "https://example.test/select.html",
        )
        .map_err(test_error)?;
        let viewport = HtmlBrowserViewport::new(320, 120, 1.0).map_err(test_error)?;
        HtmlInteractiveSession::start(source, viewport).map_err(test_error)
    }

    fn select_node_id(session: &mut HtmlInteractiveSession, id: &str) -> Result<u64, String> {
        session
            .runtime
            .node_for_element_id(id)
            .map(|node| node.0)
            .ok_or(format!("{id} select node is missing"))
    }

    fn select_box(
        session: &HtmlInteractiveSession,
        id: u64,
    ) -> Result<&super::super::types::ElementBox, String> {
        session
            .element_boxes
            .iter()
            .find(|element| element.node_id == id)
            .ok_or(format!("select box {id} is missing"))
    }

    fn assert_hit_geometry(
        target: &super::super::types::HitTarget,
        element: &super::super::types::ElementBox,
    ) {
        assert_eq!(
            (target.x, target.y, target.width, target.height),
            (element.x, element.y, element.width, element.height)
        );
    }

    #[test]
    fn parsed_select_layout_preserves_explicit_size_nested_selection_and_hit_bounds()
    -> Result<(), String> {
        let mut session = parsed_select_session()?;
        let layout = session.layout().map_err(test_error)?;
        let explicit_id = select_node_id(&mut session, "explicit")?;
        let default_id = select_node_id(&mut session, "default")?;
        let explicit = select_box(&session, explicit_id)?;
        let fallback = select_box(&session, default_id)?;
        assert_eq!((explicit.width, explicit.height), (180.0, 44.0));
        assert_ne!(fallback.width, explicit.width);
        assert_ne!(fallback.height, explicit.height);
        assert!(layout.svg.contains(">chosen</text>"), "{}", layout.svg);
        assert!(layout.svg.contains(">fallback</text>"), "{}", layout.svg);
        let explicit_target = layout
            .hit_targets
            .iter()
            .find(|target| target.node_id == explicit_id)
            .ok_or("explicit select hit target is missing".to_string())?;
        assert_hit_geometry(explicit_target, explicit);
        assert!(explicit_target.width > fallback.width);
        Ok(())
    }

    #[test]
    fn parsed_empty_select_keeps_click_geometry_without_a_label() -> Result<(), String> {
        const EXPECTED_LABELS: usize = 2;
        const AUTHORED_EMPTY_WIDTH: f32 = 80.0;
        let mut session = parsed_select_session()?;
        let layout = session.layout().map_err(test_error)?;
        let id = select_node_id(&mut session, "empty")?;
        let element = select_box(&session, id)?;
        let target = layout
            .hit_targets
            .iter()
            .find(|target| target.node_id == id)
            .ok_or("empty select hit target is missing")?;
        assert_eq!(
            (element.width, element.height),
            (AUTHORED_EMPTY_WIDTH, SELECT_HEIGHT)
        );
        assert_hit_geometry(target, element);
        assert_eq!(layout.svg.matches("</text>").count(), EXPECTED_LABELS);
        Ok(())
    }

    #[test]
    fn test_error_preserves_real_invalid_viewport() -> Result<(), String> {
        super::super::tests::assert_invalid_viewport_error()
    }

    #[test]
    fn select_helpers_report_missing_nodes_and_boxes() -> Result<(), String> {
        let mut session = parsed_select_session()?;
        assert_eq!(
            select_node_id(&mut session, "absent").err(),
            Some("absent select node is missing".to_string())
        );
        let node = select_node_id(&mut session, "explicit")?;
        session.element_boxes.clear();
        assert_eq!(
            select_box(&session, node).err(),
            Some(format!("select box {node} is missing"))
        );
        Ok(())
    }
}
