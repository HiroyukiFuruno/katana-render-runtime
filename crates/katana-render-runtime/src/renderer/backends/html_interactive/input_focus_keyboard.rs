use super::super::super::html_runtime::{HtmlNodeId, HtmlRuntimeEvent};
use super::{HtmlBrowserError, HtmlInteractiveSession};

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends::html_interactive) fn dispatch_key_down(
        &mut self,
        key: String,
    ) -> Result<(), HtmlBrowserError> {
        if !matches!(key.as_str(), "Shift" | "Control" | "Alt" | "Meta") {
            self.keyboard_modality = true;
            self.focus_visible = self.focused_node.is_some();
        }
        if key == "Shift" {
            self.shift_pressed = true;
        }
        let target = self
            .focused_node
            .or_else(|| self.runtime.body_node().map(|node| node.0));
        if key == "Tab" {
            return self.dispatch_tab_keydown(target);
        }
        let prevented = if let Some(node_id) = target {
            self.dispatch_keydown_event(node_id, key.clone())?
        } else {
            false
        };
        if !prevented && key == "Enter" && self.focused_input.is_some() {
            self.commit_focused_input(true)?;
        }
        Ok(())
    }

    pub(in crate::renderer::backends::html_interactive) fn dispatch_key_up(
        &mut self,
        key: String,
    ) -> Result<(), HtmlBrowserError> {
        if key == "Shift" {
            self.shift_pressed = false;
        }
        let target = self
            .focused_node
            .or_else(|| self.runtime.body_node().map(|node| node.0));
        target.map_or(Ok(()), |node_id| {
            self.dispatch_runtime_event(HtmlRuntimeEvent::KeyUp {
                target: HtmlNodeId(node_id),
                key,
            })
        })
    }

    fn dispatch_keydown_event(
        &mut self,
        node_id: u64,
        key: String,
    ) -> Result<bool, HtmlBrowserError> {
        let prevented = self.dispatch_keydown_without_frame(node_id, key)?;
        self.render_frame()?;
        Ok(prevented)
    }

    fn dispatch_tab_keydown(&mut self, target: Option<u64>) -> Result<(), HtmlBrowserError> {
        let prevented = match target {
            Some(node_id) => self.dispatch_keydown_without_frame(node_id, "Tab".to_string())?,
            None => false,
        };
        if !prevented {
            let nodes = self.project_nodes()?;
            (self.tab_order, self.pointer_focusable_nodes) = super::focus_candidates(&nodes);
            self.advance_tab_focus(false)?;
        }
        self.render_frame()
    }

    fn dispatch_keydown_without_frame(
        &mut self,
        node_id: u64,
        key: String,
    ) -> Result<bool, HtmlBrowserError> {
        let (_, prevented) = self
            .runtime
            .dispatch_with_default_prevented(HtmlRuntimeEvent::KeyDown {
                target: HtmlNodeId(node_id),
                key,
            })
            .map_err(super::super::runtime_failure)?;
        Ok(prevented)
    }

    fn advance_tab_focus(&mut self, render: bool) -> Result<(), HtmlBrowserError> {
        let next =
            super::tab::next_candidate(&self.tab_order, self.focused_node, self.shift_pressed);
        match next {
            Some(node_id) => self.focus_node(node_id, false, render),
            None => self.blur_focused_node(render),
        }
    }
}
