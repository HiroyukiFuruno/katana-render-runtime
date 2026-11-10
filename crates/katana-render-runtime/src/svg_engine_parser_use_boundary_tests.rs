use super::center_pixel;

#[test]
fn recursive_use_references_leave_the_visible_sibling_unchanged() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><g id="group"><rect width="8" height="8" fill="#ff0000"/><use href="#group"/></g></defs>
      <use id="recursive" href="#recursive"/>
      <rect width="8" height="8" fill="#00ff00"/><use href="#group"/></svg>"##;
    assert_eq!(center_pixel(source, &[])?, [0, 255, 0, 255]);
    Ok(())
}

#[test]
fn nested_use_of_distinct_definition_remains_visible() -> Result<(), String> {
    let source = r##"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
      <defs><rect id="tile" width="8" height="8" fill="blue"/>
      <g id="group"><use href="#tile"/></g></defs><use href="#group"/></svg>"##;
    assert_eq!(center_pixel(source, &[])?, [0, 0, 255, 255]);
    Ok(())
}
