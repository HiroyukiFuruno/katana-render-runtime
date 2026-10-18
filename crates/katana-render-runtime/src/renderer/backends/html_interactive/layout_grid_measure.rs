use super::super::html_document::HtmlDocumentNode;
use super::constants::MIN_LAYOUT_WIDTH;
use super::layout_flow_measure::{intrinsic_text_width, min_content_text_width};
use super::style::{CssGridTrack, CssGridTrackBreadth, CssStyle};

pub(super) fn grid_track_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    width: f32,
    parent: &CssStyle,
) -> Vec<f32> {
    let tracks = &parent.grid_template_columns;
    let count = tracks.len().max(1);
    let column_gap = parent.flow_column_gap.unwrap_or(parent.gap);
    let available = (width - column_gap * count.saturating_sub(1) as f32).max(MIN_LAYOUT_WIDTH);
    let intrinsic = intrinsic_track_widths(nodes, styles, tracks, count);
    let mut widths = initial_grid_widths(tracks, &intrinsic, available);
    if widths.is_empty() {
        return vec![available];
    }
    distribute_grid_space(tracks, &mut widths, available);
    widths
}

fn intrinsic_track_widths(
    nodes: &[&HtmlDocumentNode],
    styles: &[CssStyle],
    tracks: &[CssGridTrack],
    count: usize,
) -> Vec<f32> {
    let mut intrinsic = vec![MIN_LAYOUT_WIDTH; count];
    for (index, (node, style)) in nodes.iter().zip(styles).enumerate() {
        let column = index % count;
        let measured = match tracks.get(column) {
            Some(CssGridTrack::MinContent) => min_content_text_width(node, style),
            Some(CssGridTrack::MinMax {
                min: CssGridTrackBreadth::MinContent,
                ..
            }) => min_content_text_width(node, style),
            _ => intrinsic_text_width(node, style),
        };
        intrinsic[column] = intrinsic[column].max(measured);
    }
    intrinsic
}

fn initial_grid_widths(tracks: &[CssGridTrack], intrinsic: &[f32], available: f32) -> Vec<f32> {
    tracks
        .iter()
        .enumerate()
        .map(|(index, track)| match track {
            CssGridTrack::Length(value) => *value,
            CssGridTrack::Percent(value) => available * value,
            CssGridTrack::Auto | CssGridTrack::MinContent | CssGridTrack::MaxContent => {
                intrinsic[index]
            }
            CssGridTrack::Fraction(_) => 0.0,
            CssGridTrack::MinMax { min, .. } => {
                grid_breadth_width(*min, intrinsic[index], available)
            }
        })
        .collect()
}

fn distribute_grid_space(tracks: &[CssGridTrack], widths: &mut [f32], available: f32) {
    let fractions = tracks
        .iter()
        .map(grid_fraction)
        .collect::<Vec<Option<f32>>>();
    if fractions.iter().any(Option::is_some) {
        distribute_fraction_space(&fractions, widths, available);
    } else {
        let allocated = widths.iter().sum::<f32>();
        let remaining = (available - allocated).max(0.0);
        let auto_count = tracks
            .iter()
            .filter(|track| matches!(track, CssGridTrack::Auto))
            .count();
        if auto_count > 0 {
            distribute_auto_space(tracks, widths, remaining / auto_count as f32);
        }
    }
}

fn distribute_fraction_space(fractions: &[Option<f32>], widths: &mut [f32], available: f32) {
    let mut active = active_fractions(fractions);
    let fixed = fixed_width(fractions, widths);
    let mut remaining = (available - fixed).max(0.0);
    while !active.is_empty() {
        let fraction_total = active.iter().map(|(_, fraction)| fraction).sum::<f32>();
        if fraction_total <= 0.0 {
            break;
        }
        let unit = remaining / fraction_total;
        let constrained = active
            .iter()
            .position(|(index, fraction)| widths[*index] > unit * *fraction);
        let Some(position) = constrained else {
            for (index, fraction) in active {
                widths[index] = unit * fraction;
            }
            return;
        };
        let (index, _) = active.swap_remove(position);
        remaining = (remaining - widths[index]).max(0.0);
    }
}

fn active_fractions(fractions: &[Option<f32>]) -> Vec<(usize, f32)> {
    fractions
        .iter()
        .enumerate()
        .filter_map(|(index, fraction)| fraction.map(|fraction| (index, fraction)))
        .collect()
}

fn fixed_width(fractions: &[Option<f32>], widths: &[f32]) -> f32 {
    widths
        .iter()
        .enumerate()
        .filter(|(index, _)| fractions[*index].is_none())
        .map(|(_, width)| *width)
        .sum()
}

fn grid_fraction(track: &CssGridTrack) -> Option<f32> {
    match track {
        CssGridTrack::Fraction(value)
        | CssGridTrack::MinMax {
            max: CssGridTrackBreadth::Fraction(value),
            ..
        } => Some(*value),
        _ => None,
    }
}

fn grid_breadth_width(breadth: CssGridTrackBreadth, intrinsic: f32, available: f32) -> f32 {
    match breadth {
        CssGridTrackBreadth::Length(value) => value,
        CssGridTrackBreadth::Percent(value) => available * value,
        CssGridTrackBreadth::Auto
        | CssGridTrackBreadth::MinContent
        | CssGridTrackBreadth::MaxContent => intrinsic,
        CssGridTrackBreadth::Fraction(_) => 0.0,
    }
}

fn distribute_auto_space(tracks: &[CssGridTrack], widths: &mut [f32], extra: f32) {
    for (track, track_width) in tracks.iter().zip(widths.iter_mut()) {
        if matches!(track, CssGridTrack::Auto) {
            *track_width += extra;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CssGridTrack, CssGridTrackBreadth, CssStyle, HtmlDocumentNode, distribute_fraction_space,
        grid_breadth_width, intrinsic_track_widths,
    };

    #[test]
    fn minmax_min_content_uses_intrinsic_word_width() {
        let node = HtmlDocumentNode::Text("longest short".to_string());
        let style = CssStyle::browser_default();
        let track = CssGridTrack::MinMax {
            min: CssGridTrackBreadth::MinContent,
            max: CssGridTrackBreadth::Fraction(1.0),
        };

        let widths = intrinsic_track_widths(&[&node], &[style], &[track], 1);

        assert!(widths[0] > super::MIN_LAYOUT_WIDTH);
    }

    #[test]
    fn fraction_distribution_handles_zero_and_constrained_tracks() {
        let mut zero = [0.0];
        distribute_fraction_space(&[Some(0.0)], &mut zero, 100.0);
        assert_eq!(zero, [0.0]);

        let mut constrained = [80.0, 0.0];
        distribute_fraction_space(&[Some(1.0), Some(1.0)], &mut constrained, 100.0);
        assert_eq!(constrained, [80.0, 20.0]);
    }

    #[test]
    fn grid_breadth_resolves_percent_intrinsic_and_fraction_values() {
        assert_eq!(
            grid_breadth_width(CssGridTrackBreadth::Percent(0.25), 40.0, 200.0),
            50.0
        );
        assert_eq!(
            grid_breadth_width(CssGridTrackBreadth::MaxContent, 40.0, 200.0),
            40.0
        );
        assert_eq!(
            grid_breadth_width(CssGridTrackBreadth::Fraction(1.0), 40.0, 200.0),
            0.0
        );
    }
}
