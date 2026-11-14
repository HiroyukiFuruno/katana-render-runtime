pub(super) struct InlineFlowState {
    pub(super) x: f32,
    pub(super) width: f32,
    pub(super) cursor_x: f32,
    pub(super) y: f32,
    pub(super) bottom: f32,
    pub(super) has_items: bool,
    pub(super) pending_space_width: Option<f32>,
    pub(super) baseline_offset: Option<f32>,
    pub(super) line_height: Option<f32>,
    line_plan: Option<super::line_plan::InlineLinePlan>,
    line_index: usize,
}

impl InlineFlowState {
    pub(super) fn new(x: f32, y: f32, width: f32) -> Self {
        Self {
            x,
            width,
            cursor_x: x,
            y,
            bottom: y,
            has_items: false,
            pending_space_width: None,
            baseline_offset: None,
            line_height: None,
            line_plan: None,
            line_index: 0,
        }
    }

    pub(super) fn resolve_line_metrics(
        &mut self,
        nodes: &[super::super::super::html_document::HtmlDocumentNode],
        inherited: &super::super::style::CssStyle,
        clickable: &std::collections::HashSet<u64>,
    ) {
        let plan =
            super::line_plan::InlineLinePlan::for_run(nodes, inherited, clickable, self.width);
        self.baseline_offset = plan.metrics(0).and_then(|metrics| metrics.baseline());
        self.line_height = plan.metrics(0).and_then(|metrics| metrics.height());
        self.line_plan = Some(plan);
        self.line_index = 0;
    }

    pub(super) fn line_advance(&self, count: usize, style: &super::super::style::CssStyle) -> f32 {
        self.line_plan
            .as_ref()
            .and_then(|plan| plan.advance(self.line_index, count))
            .unwrap_or(count as f32 * style.line_height)
    }

    pub(super) fn text_line_style(
        &self,
        index: usize,
        style: &super::super::style::CssStyle,
    ) -> super::super::style::CssStyle {
        let mut resolved = style.clone();
        if let Some(metrics) = self
            .line_plan
            .as_ref()
            .and_then(|plan| plan.metrics(self.line_index + index))
        {
            resolved.text_baseline_offset = metrics.baseline();
            resolved.line_height = metrics.height().unwrap_or(style.line_height);
        }
        resolved
    }

    pub(super) fn advance_line_index(&mut self, count: usize) {
        self.line_index += count;
    }

    pub(super) fn bottom(&self) -> f32 {
        if self.has_items { self.bottom } else { self.y }
    }
}
