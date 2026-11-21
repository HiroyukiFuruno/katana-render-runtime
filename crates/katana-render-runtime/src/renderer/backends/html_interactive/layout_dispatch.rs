use super::super::html_document::HtmlDocumentNode;
use super::layout::HtmlLayoutRenderer;
use super::types::{ElementRenderContext, LayoutContext};

impl HtmlLayoutRenderer {
    pub(super) fn render_tag(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        match element.tag {
            "html" | "body" => self.render_viewport_container(element, layout),
            "main" => self.render_container_element(element.children, layout),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" => {
                self.render_container_element(element.children, layout)
            }
            "label" => self.render_label_container_element(element, layout),
            "a" => self.render_link_element(element, layout),
            "button" => self.render_button(element, layout),
            "img" => self.render_image(
                element.attributes,
                layout.x,
                layout.y,
                layout.width,
                layout.style,
            ),
            "svg" => self.render_embedded_svg(element.attributes, layout),
            "input" | "textarea" => self.render_input_element(element, layout),
            "select" => self.render_select(element, layout),
            "details" => self.render_details(element, layout),
            "summary" => self.render_summary(element, layout),
            _ => self.render_structural_tag(element, layout),
        }
    }

    fn render_structural_tag(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        match element.tag {
            "table" => self.render_table_element(element.children, layout),
            "hr" => self.render_rule_element(layout),
            "br" => layout.y + layout.style.line_height,
            "ul" | "ol" => self.render_list_element(element.children, layout, element.tag == "ol"),
            "li" => self.render_list_item_element(element.children, layout),
            _ => self.render_container_element(element.children, layout),
        }
    }

    fn render_container_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_container(
            children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            layout.details,
        )
    }

    fn render_viewport_container(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let mut style = layout.style.clone();
        style.min_height = style
            .min_height
            .max((self.viewport_height - style.margin_top - style.margin_bottom).max(0.0));
        self.render_container(
            element.children,
            layout.x,
            layout.y,
            layout.width,
            &style,
            layout.details,
        )
    }

    fn render_label_container_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_container(
            element.children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            layout.details,
        )
    }

    fn render_link_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_link(
            element.node_id,
            element.children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
        )
    }

    fn render_input_element(
        &mut self,
        element: ElementRenderContext<'_>,
        layout: LayoutContext<'_>,
    ) -> f32 {
        let bottom = self.render_input(
            element.node_id,
            element.attributes,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
        );
        self.render_input_children(element.children, layout);
        bottom
    }

    fn render_input_children(&mut self, children: &[HtmlDocumentNode], layout: LayoutContext<'_>) {
        if children.is_empty() {
            return;
        }
        let child_x = layout.x + layout.style.margin_left;
        let child_y = layout.y + layout.style.margin_top;
        let child_width = layout
            .style
            .explicit_width(layout.width)
            .unwrap_or(layout.width);
        self.render_nodes(
            children,
            child_x,
            child_y,
            child_width,
            layout.style,
            layout.details,
        );
    }

    fn render_table_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_table(children, layout.x, layout.y, layout.width, layout.style)
    }

    fn render_rule_element(&mut self, layout: LayoutContext<'_>) -> f32 {
        self.render_rule(layout.x, layout.y, layout.width, layout.style)
    }

    fn render_list_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
        ordered: bool,
    ) -> f32 {
        self.render_list(
            children,
            layout.x,
            layout.y,
            layout.width,
            layout.style,
            ordered,
        )
    }

    fn render_list_item_element(
        &mut self,
        children: &[HtmlDocumentNode],
        layout: LayoutContext<'_>,
    ) -> f32 {
        self.render_list_item(children, layout.x, layout.y, layout.width, layout.style)
    }
}
