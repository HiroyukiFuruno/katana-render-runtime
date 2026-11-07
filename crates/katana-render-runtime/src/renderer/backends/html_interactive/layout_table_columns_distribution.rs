pub(super) fn preferred_widths(widths: &[Option<f32>], minimums: &[f32]) -> Vec<f32> {
    widths
        .iter()
        .enumerate()
        .map(|(index, width)| width.map_or(minimums[index], |value| value.max(minimums[index])))
        .collect()
}

pub(super) fn specified_width(widths: &[Option<f32>], minimums: &[f32], available: f32) -> f32 {
    widths
        .iter()
        .zip(minimums)
        .filter_map(|(width, minimum)| width.map(|value| value.max(*minimum)))
        .sum::<f32>()
        .min(available)
}

pub(super) fn distribute_percentage_guess(
    widths: &[Option<f32>],
    minimums: &[f32],
    available: f32,
) -> Vec<f32> {
    let minimum_total = minimums.iter().sum::<f32>();
    let distributable = (available - minimum_total).max(0.0);
    let weights = widths
        .iter()
        .zip(minimums)
        .map(|(width, minimum)| width.map(|value| (value - minimum).max(0.0)).unwrap_or(0.0))
        .collect::<Vec<_>>();
    let total_weight = weights.iter().sum::<f32>();
    minimums
        .iter()
        .zip(weights)
        .map(|(minimum, increase)| {
            minimum
                + if total_weight > 0.0 {
                    distributable * increase / total_weight
                } else {
                    0.0
                }
        })
        .collect()
}

pub(super) fn fill_undersubscribed_declared_widths(
    widths: &[Option<f32>],
    minimums: &[f32],
    available: f32,
) -> Vec<f32> {
    let mut result = distribute_percentage_guess(widths, minimums, available);
    let remainder = (available - result.iter().sum::<f32>()).max(0.0);
    let share = remainder / result.len() as f32;
    result.iter_mut().for_each(|width| *width += share);
    result
}

pub(super) fn all_widths_declared(widths: &[Option<f32>]) -> bool {
    widths.iter().all(Option::is_some)
}

pub(super) fn declared_widths_need_remainder(
    widths: &[Option<f32>],
    preferred: &[f32],
    available: f32,
) -> bool {
    !widths.is_empty() && all_widths_declared(widths) && preferred.iter().sum::<f32>() < available
}

pub(super) fn declared_width_values(widths: Vec<Option<f32>>) -> Vec<f32> {
    widths.into_iter().flatten().collect()
}

pub(super) fn enforce_minimum_widths(
    preferred: Vec<f32>,
    minimums: Vec<f32>,
    available: f32,
) -> Vec<f32> {
    let mut deficit = (preferred.iter().sum::<f32>() - available).max(0.0);
    let mut result = preferred;
    loop {
        let active = active_shrink_width(&result, &minimums);
        if active <= 0.0 || deficit <= 0.0 {
            break;
        }
        let mut removed = 0.0;
        for (value, minimum) in result.iter_mut().zip(&minimums) {
            let available_reduction = (*value - *minimum).max(0.0);
            if available_reduction <= 0.0 {
                continue;
            }
            let reduction = (deficit * *value / active).min(available_reduction);
            *value -= reduction;
            removed += reduction;
        }
        if removed <= f32::EPSILON {
            break;
        }
        deficit -= removed;
    }
    result
}

fn active_shrink_width(widths: &[f32], minimums: &[f32]) -> f32 {
    widths
        .iter()
        .zip(minimums)
        .filter(|(value, minimum)| **value > **minimum)
        .map(|(value, _)| *value)
        .sum::<f32>()
}

pub(super) fn resolve_flexible_widths(
    widths: Vec<Option<f32>>,
    minimums: Vec<f32>,
    available: f32,
    specified: f32,
) -> Vec<f32> {
    let flexible = widths.iter().filter(|width| width.is_none()).count();
    let intrinsic_flexible = widths
        .iter()
        .enumerate()
        .filter(|(_, width)| width.is_none())
        .map(|(index, _)| minimums[index])
        .sum::<f32>();
    let flexible_target = (available - specified).max(0.0);
    widths
        .into_iter()
        .enumerate()
        .map(|(index, width)| {
            resolve_column_width(
                width,
                minimums[index],
                flexible,
                flexible_target,
                intrinsic_flexible,
            )
        })
        .collect()
}

fn resolve_column_width(
    width: Option<f32>,
    minimum: f32,
    flexible: usize,
    flexible_target: f32,
    intrinsic_flexible: f32,
) -> f32 {
    width.map_or_else(
        || match (flexible, intrinsic_flexible > 0.0) {
            (0, _) => minimum,
            (_, true) => flexible_target * minimum / intrinsic_flexible,
            (_, false) => flexible_target / flexible as f32,
        },
        |specified_width| specified_width.max(minimum),
    )
}
