use super::super::html_runtime::{HtmlNodeId, HtmlRuntimeEvent};
use super::{HtmlBrowserError, HtmlInteractiveSession};
use std::collections::HashSet;

#[cfg(test)]
#[path = "input_focus_tests.rs"]
mod tests;

#[path = "input_focus_tab.rs"]
mod tab;

impl HtmlInteractiveSession {
    pub(super) fn update_focus(&mut self, focused: bool) -> Result<(), HtmlBrowserError> {
        if !focused {
            self.blur_focused_node(true)?;
        }
        Ok(())
    }

    pub(super) fn focus_input_without_frame(
        &mut self,
        node_id: u64,
    ) -> Result<(), HtmlBrowserError> {
        self.focus_node(node_id, true, false)?;
        self.input_values.entry(node_id).or_default();
        Ok(())
    }

    pub(super) fn blur_focused_node_without_frame(&mut self) -> Result<(), HtmlBrowserError> {
        self.blur_focused_node(false)
    }

    fn focus_node(
        &mut self,
        node_id: u64,
        pointer: bool,
        render: bool,
    ) -> Result<(), HtmlBrowserError> {
        self.keyboard_modality = !pointer;
        if self.focused_node == Some(node_id) {
            return self.refresh_focus_visibility(render);
        }
        self.blur_focused_node(render)?;
        self.set_focused_node(node_id, render)
    }

    fn refresh_focus_visibility(&mut self, render: bool) -> Result<(), HtmlBrowserError> {
        let focus_visible = self.keyboard_modality || self.focused_input.is_some();
        if self.focus_visible != focus_visible {
            self.focus_visible = focus_visible;
            if render {
                self.render_frame()?;
            }
        }
        Ok(())
    }

    fn set_focused_node(&mut self, node_id: u64, render: bool) -> Result<(), HtmlBrowserError> {
        self.focused_node = Some(node_id);
        self.focused_input = self
            .runtime
            .is_text_control(node_id)
            .map_err(super::runtime_failure)?
            .then_some(node_id);
        self.focus_visible = self.keyboard_modality || self.focused_input.is_some();
        self.runtime
            .set_active_element(Some(node_id))
            .map_err(super::runtime_failure)?;
        self.dispatch_focus_event(
            HtmlRuntimeEvent::Focus {
                target: HtmlNodeId(node_id),
            },
            render,
        )?;
        Ok(())
    }

    fn blur_focused_node(&mut self, render: bool) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_node else {
            return Ok(());
        };
        self.commit_focused_input(render)?;
        self.runtime
            .set_active_element(None)
            .map_err(super::runtime_failure)?;
        self.focused_node = None;
        self.focused_input = None;
        self.focus_visible = false;
        self.dispatch_focus_event(
            HtmlRuntimeEvent::Blur {
                target: HtmlNodeId(node_id),
            },
            render,
        )
    }

    pub(super) fn dispatch_key_down(&mut self, key: String) -> Result<(), HtmlBrowserError> {
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

    pub(super) fn dispatch_key_up(&mut self, key: String) -> Result<(), HtmlBrowserError> {
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
            (self.tab_order, self.pointer_focusable_nodes) = focus_candidates(&nodes);
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
            .map_err(super::runtime_failure)?;
        Ok(prevented)
    }

    fn advance_tab_focus(&mut self, render: bool) -> Result<(), HtmlBrowserError> {
        let next = tab::next_candidate(&self.tab_order, self.focused_node, self.shift_pressed);
        match next {
            Some(node_id) => self.focus_node(node_id, false, render),
            None => self.blur_focused_node(render),
        }
    }

    pub(super) fn prepare_pointer_focus(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        if self.pointer_focusable_nodes.contains(&node_id) {
            self.focus_node(node_id, true, false)?;
        } else {
            self.blur_focused_node(false)?;
        }
        Ok(())
    }

    fn commit_focused_input(&mut self, render: bool) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        if self.dirty_inputs.remove(&node_id) {
            let event = HtmlRuntimeEvent::Change {
                target: HtmlNodeId(node_id),
            };
            return self.dispatch_focus_event(event, render);
        }
        Ok(())
    }

    fn dispatch_focus_event(
        &mut self,
        event: HtmlRuntimeEvent,
        render: bool,
    ) -> Result<(), HtmlBrowserError> {
        if render {
            return self.dispatch_runtime_event(event);
        }
        self.runtime
            .dispatch(event)
            .map_err(super::runtime_failure)?;
        Ok(())
    }
}

pub(super) fn focus_candidates(
    nodes: &[super::super::html_document::HtmlDocumentNode],
) -> (Vec<u64>, HashSet<u64>) {
    tab::focus_candidates(nodes)
}
