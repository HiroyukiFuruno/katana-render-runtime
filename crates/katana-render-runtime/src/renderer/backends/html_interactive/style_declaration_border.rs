use crate::renderer::backends::html_interactive::style::{CssStyle, normalized_property_name};

impl CssStyle {
    pub(crate) fn apply_border_declaration(&mut self, name: &str, value: &str) -> bool {
        match normalized_property_name(name).as_ref() {
            "border" => self.apply_border(value),
            "border-color" => self.apply_border_color(value),
            "border-style" => self.apply_border_style(value),
            "border-top" | "border-right" | "border-bottom" | "border-left" => {
                self.apply_border_side(name, value)
            }
            "border-top-style"
            | "border-right-style"
            | "border-bottom-style"
            | "border-left-style" => self.apply_border_side_style(name, value),
            "border-top-color"
            | "border-right-color"
            | "border-bottom-color"
            | "border-left-color" => self.apply_border_side_color(name, value),
            _ => return false,
        }
        true
    }
}
