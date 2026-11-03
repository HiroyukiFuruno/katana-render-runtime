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
