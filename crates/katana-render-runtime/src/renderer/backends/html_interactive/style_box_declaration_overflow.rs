use crate::renderer::backends::html_interactive::style::CssOverflowAxis;

pub(super) struct OverflowAssignment {
    pub(super) x: Option<CssOverflowAxis>,
    pub(super) y: Option<CssOverflowAxis>,
}

pub(super) fn parse_overflow_declaration(name: &str, value: &str) -> Option<OverflowAssignment> {
    let values: Vec<_> = value.split_whitespace().collect();
    match name {
        "overflow" => {
            let (x, y) = match values.as_slice() {
                [value] => (parse_axis(value), parse_axis(value)),
                [x, y] => (parse_axis(x), parse_axis(y)),
                _ => (None, None),
            };
            Some(OverflowAssignment { x, y })
                .filter(|assignment| assignment.x.is_some() && assignment.y.is_some())
        }
        "overflow-x" | "overflow-y" if values.len() == 1 => {
            let axis = parse_axis(values[0])?;
            Some(if name == "overflow-x" {
                OverflowAssignment {
                    x: Some(axis),
                    y: None,
                }
            } else {
                OverflowAssignment {
                    x: None,
                    y: Some(axis),
                }
            })
        }
        _ => None,
    }
}

fn parse_axis(value: &str) -> Option<CssOverflowAxis> {
    match value.to_ascii_lowercase().as_str() {
        "visible" => Some(CssOverflowAxis::Visible),
        "clip" => Some(CssOverflowAxis::Clip),
        "hidden" => Some(CssOverflowAxis::Hidden),
        "auto" => Some(CssOverflowAxis::Auto),
        "scroll" => Some(CssOverflowAxis::Scroll),
        _ => None,
    }
}
