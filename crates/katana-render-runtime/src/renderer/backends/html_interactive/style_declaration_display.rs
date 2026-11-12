use crate::renderer::backends::html_interactive::style::CssStyle;
use crate::renderer::backends::html_interactive::{document::attribute, style_declaration_cache};
use taffy::style::Display;

impl CssStyle {
    pub(crate) fn apply_display(&mut self, value: &str) {
        let Some((display, inline)) = display_value(value) else {
            return;
        };
        self.display = display;
        self.inline_block = inline.is_some();
        self.inline_atomic = inline.unwrap_or(false);
    }

    pub(in crate::renderer::backends::html_interactive) fn element_has_display_none(
        attributes: &[(String, String)],
    ) -> bool {
        let display = attribute(attributes, "style").and_then(|source| {
            style_declaration_cache::get_or_parse(source)
                .iter()
                .rev()
                .filter(|declaration| declaration.name.eq_ignore_ascii_case("display"))
                .find_map(|declaration| display_value(&declaration.value))
        });
        display.map_or_else(
            || attribute(attributes, "hidden").is_some(),
            |(display, _)| display == Display::None,
        )
    }

    pub(crate) fn apply_color(&mut self, value: &str) {
        self.color = value.to_string();
        self.explicit_color = true;
    }

    pub(crate) fn apply_background(&mut self, value: &str) {
        self.background = Some(value.to_string());
        self.explicit_background = true;
    }
}

fn display_value(value: &str) -> Option<(Display, Option<bool>)> {
    match value.trim().to_ascii_lowercase().as_str() {
        "inline" => Some((Display::Block, Some(false))),
        "inline-block" => Some((Display::Block, Some(true))),
        "inline-flex" => Some((Display::Flex, Some(true))),
        "inline-grid" => Some((Display::Grid, Some(true))),
        _ => value.parse().ok().map(|display| (display, None)),
    }
}
