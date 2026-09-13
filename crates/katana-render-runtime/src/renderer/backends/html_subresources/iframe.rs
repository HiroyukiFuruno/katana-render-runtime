use super::super::html_browser::HtmlBrowserSource;
use super::HtmlSubresourceLoader;
use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_dom_helpers::{attribute_value, detach, find_element};
use markup5ever_rcdom::{Handle, NodeData};
use std::collections::HashSet;
use std::rc::Rc;

const MAX_IFRAME_DEPTH: usize = 8;
const MAX_IFRAME_DOCUMENTS: usize = 16;
const FRAME_ATTRIBUTE: &str = "data-krr-local-frame";
const FRAME_ERROR_ATTRIBUTE: &str = "data-krr-frame-error";

pub(super) fn inline_iframes(loader: &HtmlSubresourceLoader, document: &mut HtmlDocument) {
    let mut state = LocalIframeState::default();
    inline_document(loader, document, &mut state, 0);
}

#[derive(Default)]
struct LocalIframeState {
    active_origins: HashSet<String>,
    loaded_documents: usize,
}

fn inline_document(
    loader: &HtmlSubresourceLoader,
    document: &mut HtmlDocument,
    state: &mut LocalIframeState,
    depth: usize,
) {
    let mut frames = Vec::new();
    collect_iframes(&document.document, &mut frames);
    for (frame, reference) in frames {
        let Some(reference) = reference else {
            continue;
        };
        if let Err(error) = inline_frame(loader, document, state, depth, &frame, &reference) {
            log_iframe_failure(loader, &reference, &error);
            let _diagnostic_result = attach_iframe_error(document, &frame, &reference, &error);
        }
    }
}

fn inline_frame(
    loader: &HtmlSubresourceLoader,
    document: &mut HtmlDocument,
    state: &mut LocalIframeState,
    depth: usize,
    frame: &Handle,
    reference: &str,
) -> Result<(), String> {
    validate_frame_limits(state, depth)?;
    let source = loader.load_iframe(reference)?;
    let origin = register_frame_origin(state, &source)?;
    let result = inline_loaded_frame(loader, document, state, depth, frame, &source);
    state.active_origins.remove(&origin);
    result
}

fn validate_frame_limits(state: &LocalIframeState, depth: usize) -> Result<(), String> {
    if depth >= MAX_IFRAME_DEPTH {
        return Err(format!("iframe nesting exceeds {MAX_IFRAME_DEPTH}"));
    }
    if state.loaded_documents >= MAX_IFRAME_DOCUMENTS {
        return Err(format!("iframe count exceeds {MAX_IFRAME_DOCUMENTS}"));
    }
    Ok(())
}

fn register_frame_origin(
    state: &mut LocalIframeState,
    source: &HtmlBrowserSource,
) -> Result<String, String> {
    let origin = source.origin.as_str().to_string();
    if !state.active_origins.insert(origin.clone()) {
        return Err("iframe cycle was rejected".to_string());
    }
    state.loaded_documents += 1;
    Ok(origin)
}

fn inline_loaded_frame(
    loader: &HtmlSubresourceLoader,
    document: &mut HtmlDocument,
    state: &mut LocalIframeState,
    depth: usize,
    frame: &Handle,
    source: &HtmlBrowserSource,
) -> Result<(), String> {
    let mut child = HtmlDocument::parse(&source.raw_html);
    inline_document(loader, &mut child, state, depth + 1);
    let child_root = required_html_root(&child.document, "iframe document")?;
    attach_child_document(document, frame, child_root, FRAME_ATTRIBUTE)
}

fn attach_child_document(
    document: &mut HtmlDocument,
    frame: &Handle,
    child_root: Handle,
    frame_attribute: &str,
) -> Result<(), String> {
    let old_children = std::mem::take(&mut *frame.children.borrow_mut());
    for child in old_children {
        child.parent.set(None);
    }
    detach(&child_root);
    child_root.parent.set(Some(Rc::downgrade(frame)));
    frame.children.borrow_mut().push(child_root.clone());
    document.register_subtree(&child_root);
    let frame_id = document.register_subtree(frame);
    document.set_attribute(frame_id, frame_attribute, "")
}

fn collect_iframes(node: &Handle, frames: &mut Vec<(Handle, Option<String>)>) {
    if let NodeData::Element { name, attrs, .. } = &node.data
        && name.local.as_str().eq_ignore_ascii_case("iframe")
    {
        frames.push((
            node.clone(),
            attribute_value(&attrs.borrow(), "src").map(ToOwned::to_owned),
        ));
        return;
    }
    for child in node.children.borrow().iter() {
        collect_iframes(child, frames);
    }
}

fn attach_iframe_error(
    document: &mut HtmlDocument,
    frame: &Handle,
    reference: &str,
    error: &str,
) -> Result<(), String> {
    let message = format!(
        "<div class=krr-frame-error style='box-sizing:border-box;border:2px solid #dc2626;\
         padding:16px;background:#fff4f4;color:#7f1d1d;font:16px sans-serif;\
         line-height:1.5;overflow-wrap:anywhere'><strong>HTML iframe could not be loaded.</strong>\
         <br>{}<br>{}</div>",
        escape_text(reference),
        escape_text(error)
    );
    let child = HtmlDocument::parse(&message);
    let child_root = required_html_root(&child.document, "iframe diagnostic document")?;
    attach_child_document(document, frame, child_root, FRAME_ERROR_ATTRIBUTE)
}

pub(super) fn required_html_root(document: &Handle, context: &str) -> Result<Handle, String> {
    find_element(document, |tag, _| tag == "html")
        .ok_or_else(|| format!("{context} has no html root"))
}

fn escape_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn log_iframe_failure(loader: &HtmlSubresourceLoader, reference: &str, error: &str) {
    let document_origin = loader.document_origin();
    tracing::warn!(
        layer = "KRR runtime",
        operation = "load_iframe",
        document = document_origin,
        resource_kind = "iframe",
        resource = reference,
        error,
        "HTML iframe load failed; rendering continues"
    );
}
