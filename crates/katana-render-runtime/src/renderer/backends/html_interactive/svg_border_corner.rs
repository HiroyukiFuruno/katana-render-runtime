use super::super::RoundedBorderGeometry;
use crate::renderer::backends::html_interactive::constants::BORDER_RADIUS_CORNER_COUNT;

const QUADRATIC_DISCRIMINANT_FACTOR: f32 = 4.0;

pub(super) struct CornerArc {
    pub(super) outer_radius: (f32, f32),
    pub(super) inner_radius: (f32, f32),
    pub(super) outer_split: (f32, f32),
    pub(super) inner_split: (f32, f32),
    pub(super) outer_incoming: (f32, f32),
    pub(super) outer_outgoing: (f32, f32),
    pub(super) inner_incoming: (f32, f32),
    pub(super) inner_outgoing: (f32, f32),
}

#[derive(Clone, Copy)]
struct CornerSpec {
    origin: (f32, f32),
    signs: (f32, f32),
    borders: (f32, f32),
    incoming_vertical: bool,
}

pub(super) fn rounded_corners(
    g: &RoundedBorderGeometry,
) -> [CornerArc; BORDER_RADIUS_CORNER_COUNT] {
    let origins = [
        (g.x, g.y),
        (g.x + g.width, g.y),
        (g.x + g.width, g.y + g.height),
        (g.x, g.y + g.height),
    ];
    let signs = [(1.0, 1.0), (-1.0, 1.0), (-1.0, -1.0), (1.0, -1.0)];
    let borders = [
        (g.left, g.top),
        (g.right, g.top),
        (g.right, g.bottom),
        (g.left, g.bottom),
    ];
    let specs: [CornerSpec; BORDER_RADIUS_CORNER_COUNT] = std::array::from_fn(|index| CornerSpec {
        origin: origins[index],
        signs: signs[index],
        borders: borders[index],
        incoming_vertical: index % 2 == 0,
    });
    std::array::from_fn(|index| corner_arc(specs[index], g.outer[index], g.inner[index]))
}

fn corner_arc(spec: CornerSpec, outer: (f32, f32), inner: (f32, f32)) -> CornerArc {
    let outer_tangents = tangents(&spec, outer, (0.0, 0.0));
    let inner_tangents = tangents(&spec, inner, spec.borders);
    CornerArc {
        outer_radius: outer,
        inner_radius: inner,
        outer_split: absolute_point(&spec, ellipse_split(spec.borders, outer, (0.0, 0.0))),
        inner_split: absolute_point(&spec, ellipse_split(spec.borders, inner, spec.borders)),
        outer_incoming: outer_tangents.0,
        outer_outgoing: outer_tangents.1,
        inner_incoming: inner_tangents.0,
        inner_outgoing: inner_tangents.1,
    }
}

fn tangents(spec: &CornerSpec, radius: (f32, f32), inset: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let horizontal = absolute_point(spec, (inset.0 + radius.0, inset.1));
    let vertical = absolute_point(spec, (inset.0, inset.1 + radius.1));
    if spec.incoming_vertical {
        (vertical, horizontal)
    } else {
        (horizontal, vertical)
    }
}

fn absolute_point(spec: &CornerSpec, local: (f32, f32)) -> (f32, f32) {
    (
        spec.origin.0 + spec.signs.0 * local.0,
        spec.origin.1 + spec.signs.1 * local.1,
    )
}

fn ellipse_split(borders: (f32, f32), radius: (f32, f32), inset: (f32, f32)) -> (f32, f32) {
    if radius.0 <= 0.0 || radius.1 <= 0.0 {
        return inset;
    }
    let direction = if borders == (0.0, 0.0) {
        (1.0, 1.0)
    } else {
        borders
    };
    /* WHY: 外角と内角を結ぶ境界線を楕円まで延ばし、辺の閉じ線が角の内側を塗らないようにする。 */
    let dx = direction.0 / radius.0;
    let dy = direction.1 / radius.1;
    let cx = (inset.0 + radius.0) / radius.0;
    let cy = (inset.1 + radius.1) / radius.1;
    let a = dx * dx + dy * dy;
    let b = -2.0 * (dx * cx + dy * cy);
    let c = cx * cx + cy * cy - 1.0;
    let discriminant = (b * b - QUADRATIC_DISCRIMINANT_FACTOR * a * c).max(0.0);
    let parameter = (-b - discriminant.sqrt()) / (2.0 * a);
    (parameter * direction.0, parameter * direction.1)
}

#[cfg(test)]
mod tests {
    use super::ellipse_split;

    const RADIUS: f32 = 8.0;
    const WIDE_BORDER: f32 = 4.0;
    const EPSILON: f32 = 0.0001;

    #[test]
    fn unequal_widths_split_the_ellipse_on_the_border_diagonal() {
        let point = ellipse_split((WIDE_BORDER, 1.0), (RADIUS, RADIUS), (0.0, 0.0));
        let ellipse = ((point.0 - RADIUS) / RADIUS).powi(2) + ((point.1 - RADIUS) / RADIUS).powi(2);
        assert!((ellipse - 1.0).abs() < EPSILON);
        assert!((point.0 - WIDE_BORDER * point.1).abs() < EPSILON);
    }

    #[test]
    fn zero_width_and_degenerate_corners_have_finite_endpoints() {
        assert_eq!(
            ellipse_split((WIDE_BORDER, 0.0), (RADIUS, RADIUS), (0.0, 0.0)),
            (RADIUS, 0.0)
        );
        assert_eq!(
            ellipse_split((WIDE_BORDER, 1.0), (0.0, RADIUS), (WIDE_BORDER, 1.0)),
            (WIDE_BORDER, 1.0)
        );
        let point = ellipse_split((0.0, 0.0), (RADIUS, RADIUS), (0.0, 0.0));
        assert!(point.0.is_finite() && point.1.is_finite());
        assert!((point.0 - point.1).abs() < EPSILON);
    }
}
