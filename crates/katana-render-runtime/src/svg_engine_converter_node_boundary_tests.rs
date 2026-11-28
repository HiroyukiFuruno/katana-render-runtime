use super::super::{Cache, Group, NonZeroRect, Options, convert_element, root_state, svgtree};
use std::{cell::RefCell, rc::Rc, sync::Arc};

fn assert_conversion(document: &svgtree::Document<'_>) -> Result<(), String> {
    let text = document
        .descendants()
        .find(|node| node.is_text() && node.text() == "sample")
        .ok_or("text node missing")?;
    let rect = document
        .descendants()
        .find(|node| node.element_id() == "control")
        .ok_or("rect node missing")?;
    let fontdb = Arc::new(fontdb::Database::new());
    let shared = Rc::new(RefCell::new(fontdb.clone()));
    let mut cache = Cache::new(fontdb, shared.clone());
    let options = Options::default();
    let view_box = NonZeroRect::from_xywh(0.0, 0.0, 100.0, 100.0).ok_or("invalid view box")?;
    let state = root_state(view_box, &options, shared);
    let mut group = Group::empty();
    let before = format!("{group:?}");
    convert_element(text, &state, &mut cache, &mut group);
    assert_eq!(format!("{group:?}"), before);
    convert_element(rect, &state, &mut cache, &mut group);
    assert_eq!(group.children().len(), 1);
    Ok(())
}

#[test]
fn text_node_is_ignored_while_rect_is_converted() -> Result<(), String> {
    let source = "<svg xmlns='http://www.w3.org/2000/svg'><text>sample</text><rect id='control' width='10' height='10'/></svg>";
    let xml = roxmltree::Document::parse(source).map_err(|error| error.to_string())?;
    let document = svgtree::Document::parse_tree(&xml, None).map_err(|error| error.to_string())?;
    assert_conversion(&document)
}
