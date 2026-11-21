use super::{Mode, SCALE, marker_path_count, parse, render};

const RECURSIVE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40">
<defs>
  <marker id="a" markerWidth="10" markerHeight="10" markerUnits="userSpaceOnUse" overflow="visible">
    <path d="M0 0 L8 0 L0 8 Z" fill="blue" marker-start="url(#b)"/>
  </marker>
  <marker id="b" markerWidth="6" markerHeight="6" markerUnits="userSpaceOnUse" overflow="visible">
    <path d="M0 0 L5 0 L0 5 Z" fill="red" marker-start="url(#a)"/>
  </marker>
</defs>
<path id="source" d="M10 10 L50 10" stroke="black" fill="none" marker-start="url(#a)"/>
</svg>"##;

#[test]
fn mutually_recursive_markers_match_the_explicitly_terminated_chain() -> Result<(), String> {
    let terminated = RECURSIVE.replacen("fill=\"red\" marker-start=\"url(#a)\"", "fill=\"red\"", 1);
    let outer_only =
        terminated.replacen("fill=\"blue\" marker-start=\"url(#b)\"", "fill=\"blue\"", 1);
    assert_ne!(RECURSIVE, terminated);
    assert_ne!(terminated, outer_only);
    let cyclic_tree = parse(RECURSIVE)?;
    let finite_tree = parse(&terminated)?;
    let outer_tree = parse(&outer_only)?;
    assert_eq!(marker_path_count(cyclic_tree.root()), 2);
    assert_eq!(marker_path_count(finite_tree.root()), 2);
    assert_eq!(marker_path_count(outer_tree.root()), 1);
    let cyclic = render(&cyclic_tree, Mode::Extra(SCALE))?;
    let finite = render(&finite_tree, Mode::Extra(SCALE))?;
    let outer = render(&outer_tree, Mode::Extra(SCALE))?;
    assert_eq!(cyclic.data(), finite.data());
    assert_ne!(finite.data(), outer.data());
    Ok(())
}

#[test]
fn symbol_use_inside_marker_matches_direct_geometry_without_duplicating_ids() -> Result<(), String>
{
    let symbol = r##"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="40">
      <defs><symbol id="shape" viewBox="0 0 10 10"><rect x="1" y="1" width="8" height="8" fill="red"/></symbol>
        <marker id="m" markerWidth="10" markerHeight="10" markerUnits="userSpaceOnUse" overflow="visible">
          <use id="instance" href="#shape" width="10" height="10"/>
        </marker></defs><path d="M10 10 L50 10" stroke="black" marker-start="url(#m)"/></svg>"##;
    let direct = symbol.replace(
        "<use id=\"instance\" href=\"#shape\" width=\"10\" height=\"10\"/>",
        "<rect x=\"1\" y=\"1\" width=\"8\" height=\"8\" fill=\"red\"/>",
    );
    let unmarked = direct.replace(" marker-start=\"url(#m)\"", "");
    assert_ne!(symbol, direct);
    assert_ne!(direct, unmarked);
    let symbol_tree = parse(symbol)?;
    assert!(symbol_tree.node_by_id("instance").is_none());
    let actual = render(&symbol_tree, Mode::Extra(SCALE))?;
    let expected = render(&parse(&direct)?, Mode::Extra(SCALE))?;
    let without_marker = render(&parse(&unmarked)?, Mode::Extra(SCALE))?;
    assert_eq!(actual.data(), expected.data());
    assert_ne!(actual.data(), without_marker.data());
    Ok(())
}
