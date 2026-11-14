use super::CssSelector;

#[test]
fn focus_pseudo_classes_follow_focused_and_visible_state() -> Result<(), String> {
    let focus = CssSelector::parse("button:focus").ok_or("focus selector must parse")?;
    let visible = CssSelector::parse("button:focus-visible").ok_or("focus-visible selector")?;

    assert!(focus.matches_at_focus_state("button", &Vec::new(), &[], true, false));
    assert!(!focus.matches_at_focus_state("button", &Vec::new(), &[], false, false));
    assert!(visible.matches_at_focus_state("button", &Vec::new(), &[], true, true));
    assert!(!visible.matches_at_focus_state("button", &Vec::new(), &[], true, false));
    assert_eq!(focus.specificity(), 11);
    assert_eq!(visible.specificity(), 11);
    Ok(())
}
