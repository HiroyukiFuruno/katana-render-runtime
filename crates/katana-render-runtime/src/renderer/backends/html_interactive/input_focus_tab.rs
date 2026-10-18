#[path = "input_focus_tab_candidates.rs"]
mod candidates;
#[cfg(test)]
#[path = "input_focus_tab_tests.rs"]
mod tests;

pub(super) use candidates::focus_candidates;

pub(super) fn next_candidate(
    tab_order: &[u64],
    focused_node: Option<u64>,
    backwards: bool,
) -> Option<u64> {
    if tab_order.is_empty() {
        return None;
    }
    let index = tab_order
        .iter()
        .position(|node_id| Some(*node_id) == focused_node);
    match (index, backwards) {
        (None, false) => tab_order.first().copied(),
        (None, true) => tab_order.last().copied(),
        (Some(index), false) => tab_order
            .get(index + 1)
            .or_else(|| tab_order.first())
            .copied(),
        (Some(index), true) => index
            .checked_sub(1)
            .and_then(|previous| tab_order.get(previous))
            .or_else(|| tab_order.last())
            .copied(),
    }
}
