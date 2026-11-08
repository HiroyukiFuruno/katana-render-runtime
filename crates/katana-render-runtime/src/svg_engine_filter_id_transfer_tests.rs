use base64::{Engine as _, engine::general_purpose::STANDARD};

use super::super::transfer_image_node_id;
use crate::svg_engine::{usvg, writer_tests};
use usvg::{Group, ImageKind, Node, Tree};

const PNG: &[u8] =
    include_bytes!("../../../tests/fixtures/svg-engine/upstream/resvg/tests/resources/image.png");
const SOURCE_ID: &str = "transfer-source";
const SENTINEL_ID: &str = "untouched-sibling";
const WRAPPER_ID: &str = "existing-wrapper";

fn fixture_tree() -> Result<Tree, String> {
    let png = STANDARD.encode(PNG);
    let source = format!(
        "<svg xmlns='http://www.w3.org/2000/svg' width='32' height='32'><g id='group' opacity='0.5'><rect width='2' height='2'/></g><rect id='path' width='2' height='2'/><image id='image' width='2' height='2' href='data:image/png;base64,{png}'/><text id='text' x='0' y='20' font-family='Noto Sans' font-size='8'>A</text></svg>"
    );
    let options = usvg::Options {
        fontdb: writer_tests::upstream_writer_font_database()?,
        ..usvg::Options::default()
    };
    Tree::from_str(&source, &options).map_err(|error| error.to_string())
}

fn node_by_id(tree: &Tree, id: &str) -> Result<Node, String> {
    tree.node_by_id(id)
        .cloned()
        .ok_or_else(|| format!("fixture node missing: {id}"))
}

fn png_image(group: &Group) -> Option<&Node> {
    group.children().iter().find_map(|node| match node {
        Node::Image(image) if matches!(image.kind(), ImageKind::PNG(_)) => Some(node),
        Node::Group(child) => png_image(child),
        _ => None,
    })
}

fn fixture_nodes(tree: &Tree) -> Result<Vec<Node>, String> {
    let image = png_image(tree.root())
        .cloned()
        .ok_or("fixture PNG missing")?;
    Ok(vec![
        node_by_id(tree, "group")?,
        node_by_id(tree, "path")?,
        image,
        node_by_id(tree, "text")?,
    ])
}

fn set_id(node: &mut Node, id: &str) {
    let id = id.to_owned();
    match node {
        Node::Group(group) => group.id = id,
        Node::Path(path) => path.id = id,
        Node::Image(image) => image.id = id,
        Node::Text(text) => text.id = id,
    }
}

fn wrapped(child: Node, sibling: Node) -> Group {
    let mut wrapper = Group::empty();
    wrapper.children = vec![child, sibling];
    let mut root = Group::empty();
    root.children.push(Node::Group(Box::new(wrapper)));
    root
}

fn png_bytes(node: &Node) -> Result<&[u8], String> {
    let Node::Image(image) = node else {
        return Err("expected image node".to_owned());
    };
    let ImageKind::PNG(bytes) = image.kind() else {
        return Err("expected PNG image".to_owned());
    };
    Ok(bytes.as_slice())
}

fn assert_transferred(node: &Node, sibling: &Node) -> Result<(), String> {
    let mut original = node.clone();
    set_id(&mut original, SOURCE_ID);
    let is_png = matches!(&original, Node::Image(_));
    let mut root = wrapped(original, sibling.clone());
    transfer_image_node_id(&mut root);
    assert_transferred_root(&root, sibling, is_png)
}

fn assert_transferred_root(root: &Group, sibling: &Node, is_png: bool) -> Result<(), String> {
    let Some(Node::Group(wrapper)) = root.children.first() else {
        return Err("wrapper missing".to_owned());
    };
    let [child, second] = wrapper.children.as_slice() else {
        return Err("wrapper children changed".to_owned());
    };
    assert_eq!(wrapper.id, SOURCE_ID);
    assert_eq!(child.id(), "");
    assert_eq!(format!("{second:?}"), format!("{sibling:?}"));
    if is_png {
        assert_eq!(png_bytes(child)?, PNG);
    }
    Ok(())
}

#[test]
fn real_model_nodes_transfer_ids_without_mutating_original_tree() -> Result<(), String> {
    let tree = fixture_tree()?;
    let before = format!("{:?}", tree.root());
    let nodes = fixture_nodes(&tree)?;
    let [
        Node::Group(_),
        Node::Path(path),
        Node::Image(image),
        Node::Text(_),
    ] = nodes.as_slice()
    else {
        return Err("fixture must contain group, path, PNG image, and text".to_owned());
    };
    let ImageKind::PNG(bytes) = image.kind() else {
        return Err("fixture image is not PNG".to_owned());
    };
    assert_eq!(bytes.as_slice(), PNG);
    let mut sibling = Node::Path(path.clone());
    set_id(&mut sibling, SENTINEL_ID);
    for node in &nodes {
        assert_transferred(node, &sibling)?;
    }
    assert_eq!(format!("{:?}", tree.root()), before);
    assert_eq!(
        png_bytes(png_image(tree.root()).ok_or("original PNG missing")?)?,
        PNG
    );
    Ok(())
}

fn assert_noop(mut root: Group) {
    let before = format!("{root:?}");
    transfer_image_node_id(&mut root);
    assert_eq!(format!("{root:?}"), before);
}

fn named_wrapper(path: &Node) -> Result<Group, String> {
    let mut root = wrapped(path.clone(), path.clone());
    let Some(Node::Group(wrapper)) = root.children.first_mut() else {
        return Err("named wrapper missing".to_owned());
    };
    wrapper.id = WRAPPER_ID.to_owned();
    Ok(root)
}

#[test]
fn missing_or_ineligible_first_nodes_leave_tree_unchanged() -> Result<(), String> {
    let tree = fixture_tree()?;
    let path = node_by_id(&tree, "path")?;
    let mut non_group = Group::empty();
    non_group.children.push(path.clone());
    let mut empty_wrapper = Group::empty();
    empty_wrapper
        .children
        .push(Node::Group(Box::new(Group::empty())));
    let later_wrapper = wrapped(path.clone(), path.clone());
    non_group.children.extend(later_wrapper.children.clone());
    empty_wrapper.children.extend(later_wrapper.children);
    let named = named_wrapper(&path)?;
    let mut empty_id = path.clone();
    set_id(&mut empty_id, "");
    for root in [
        Group::empty(),
        non_group,
        empty_wrapper,
        named,
        wrapped(empty_id, path),
    ] {
        assert_noop(root);
    }
    Ok(())
}
