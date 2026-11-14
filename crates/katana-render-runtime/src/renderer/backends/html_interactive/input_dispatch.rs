use super::super::super::html_browser::HtmlBrowserInput;
use super::super::super::html_runtime::{HtmlNodeId, HtmlRuntimeEvent};
use super::super::constants::LEFT_MOUSE_BUTTON;
use super::super::runtime_failure;
use super::super::types::{HitTarget, HitTargetKind};
use super::super::{HtmlBrowserError, HtmlInteractiveSession};

impl HtmlInteractiveSession {
    pub(in crate::renderer::backends) fn dispatch_input(
        &mut self,
        input: HtmlBrowserInput,
    ) -> Result<(), HtmlBrowserError> {
        input.validate()?;
        match input {
            HtmlBrowserInput::Focus { focused } => self.update_focus(focused)?,
            HtmlBrowserInput::PointerMove { x, y } => self.update_hover(x, y)?,
            HtmlBrowserInput::KeyDown { key } => self.dispatch_key_down(key)?,
            HtmlBrowserInput::KeyUp { key } => self.dispatch_key_up(key)?,
            HtmlBrowserInput::PointerDown { x, y, button } => {
                self.keyboard_modality = false;
                self.press_target(x, y, button);
            }
            HtmlBrowserInput::PointerUp { x, y, button } => self.release_target(x, y, button)?,
            HtmlBrowserInput::Scroll { delta_x, delta_y } => self.scroll(delta_x, delta_y)?,
            HtmlBrowserInput::Text { text } => self.append_text(&text)?,
        }
        Ok(())
    }

    fn update_hover(&mut self, x: f32, y: f32) -> Result<(), HtmlBrowserError> {
        let hovered_node = self.element_at(x, y).map(|element| element.node_id);
        let hovered_nodes = hovered_node.map_or_else(
            || Ok(std::collections::HashSet::new()),
            |node_id| self.runtime.node_path(node_id).map_err(runtime_failure),
        )?;
        if hovered_nodes == self.hovered_nodes {
            return Ok(());
        }
        self.hovered_nodes = hovered_nodes;
        self.render_frame()
    }

    fn press_target(&mut self, x: f32, y: f32, button: u8) {
        let hit = self.hit_target_at(x, y).cloned();
        let target_count = self.hit_targets.len();
        tracing::debug!(
            physical_x = x,
            physical_y = y,
            device_scale_factor = self.viewport.device_scale_factor,
            target_count,
            target = ?hit,
            "HTML pointer down hit-test"
        );
        self.pressed_target = (button == LEFT_MOUSE_BUTTON)
            .then(|| hit.map(|target| target.node_id))
            .flatten();
    }

    fn release_target(&mut self, x: f32, y: f32, button: u8) -> Result<(), HtmlBrowserError> {
        let result = if button == LEFT_MOUSE_BUTTON {
            self.activate_target_at(x, y)
        } else {
            Ok(())
        };
        self.pressed_target = None;
        result
    }

    fn activate_target_at(&mut self, x: f32, y: f32) -> Result<(), HtmlBrowserError> {
        let hit = self.hit_target_at(x, y).cloned();
        tracing::debug!(
            physical_x = x,
            physical_y = y,
            pressed_target = ?self.pressed_target,
            target = ?hit,
            "HTML pointer up hit-test"
        );
        let Some(target) = hit else {
            if self.pressed_target.is_none() && self.focused_node.is_some() {
                let blur = self.blur_focused_node_without_frame();
                return self.finish_pointer_update(blur);
            }
            return Ok(());
        };
        if self.pressed_target != Some(target.node_id) {
            return Ok(());
        }
        let action = self.activate_target(&target);
        self.finish_pointer_update(action)
    }

    fn activate_target(&mut self, target: &HitTarget) -> Result<(), HtmlBrowserError> {
        match target.kind {
            HitTargetKind::Checkbox => {
                self.prepare_pointer_focus(target.node_id)?;
                self.toggle_checkbox(target.node_id)
            }
            HitTargetKind::Input => {
                self.focus_input_without_frame(target.node_id)?;
                self.dispatch_click(target.node_id)
            }
            HitTargetKind::Summary { details_node_id } => {
                self.prepare_pointer_focus(target.node_id)?;
                self.toggle_details(target.node_id, details_node_id)
            }
            HitTargetKind::Click => {
                self.prepare_pointer_focus(target.node_id)?;
                self.dispatch_click(target.node_id)
            }
        }
    }

    fn toggle_checkbox(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        self.runtime
            .toggle_checked(node_id)
            .map_err(runtime_failure)?;
        self.dispatch_click(node_id)
    }

    fn toggle_details(
        &mut self,
        summary_node_id: u64,
        details_node_id: u64,
    ) -> Result<(), HtmlBrowserError> {
        self.dispatch_click(summary_node_id)?;
        self.runtime
            .toggle_open(details_node_id)
            .map_err(runtime_failure)?;
        self.dispatch_toggle(details_node_id)
    }

    fn append_text(&mut self, text: &str) -> Result<(), HtmlBrowserError> {
        let Some(node_id) = self.focused_input else {
            return Ok(());
        };
        let (state, runtime) = (&mut self.state, &mut self.runtime);
        let value = state.input_values.entry(node_id).or_default();
        value.push_str(text);
        if !text.is_empty() {
            state.dirty_inputs.insert(node_id);
        }
        runtime.set_value(node_id, value).map_err(runtime_failure)?;
        self.dispatch_runtime_event(HtmlRuntimeEvent::Input {
            target: HtmlNodeId(node_id),
        })
    }

    fn dispatch_click(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        let dispatch = self
            .runtime
            .dispatch(HtmlRuntimeEvent::Click {
                target: HtmlNodeId(node_id),
            })
            .map_err(runtime_failure)?;
        self.accept_navigation(dispatch)?;
        Ok(())
    }

    fn dispatch_toggle(&mut self, node_id: u64) -> Result<(), HtmlBrowserError> {
        self.runtime
            .dispatch(HtmlRuntimeEvent::Toggle {
                target: HtmlNodeId(node_id),
            })
            .map_err(runtime_failure)?;
        Ok(())
    }

    pub(crate) fn dispatch_runtime_event(
        &mut self,
        event: HtmlRuntimeEvent,
    ) -> Result<(), HtmlBrowserError> {
        self.runtime.dispatch(event).map_err(runtime_failure)?;
        self.render_frame()
    }

    fn finish_pointer_update(
        &mut self,
        result: Result<(), HtmlBrowserError>,
    ) -> Result<(), HtmlBrowserError> {
        result?;
        self.render_frame()
    }
}
