use super::super::html_browser::HtmlBrowserOverflowPolicy;
use super::super::html_document::HtmlDocumentNode;
use super::super::html_interactive::style::{CssOverflowAxis, CssStyle};
use taffy::style::Display;

/// viewportへ投影するoverflowの出所と軸別policy。
///
/// `source_node_id`は、paint側が元要素のclipを抑止するときに同じ投影世代を
/// 照合するためのDOM node idである。軸はCSS pixelのviewport used valueを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct HtmlViewportOverflowPolicy {
    pub(super) source_node_id: Option<u64>,
    pub(super) axes: (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy),
}

struct HtmlRootView<'a> {
    node_id: u64,
    attributes: &'a [(String, String)],
    children: &'a [HtmlDocumentNode],
}

const DEFAULT_VIEWPORT_OVERFLOW: (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy) = (
    HtmlBrowserOverflowPolicy::Auto,
    HtmlBrowserOverflowPolicy::Auto,
);

/// projected treeのroot html/bodyをCSS Overflow 3のviewport伝播規則へ写像する。
///
/// bodyはhtmlの直接の子で、displayされている最初の要素だけを対象にする。
/// root htmlの両軸がvisibleのときだけbodyのcomputed axesを採用する。
pub(super) fn viewport_overflow_policy(
    nodes: &[HtmlDocumentNode],
    inherited: &CssStyle,
) -> HtmlViewportOverflowPolicy {
    let Some(root) = find_html_node(nodes) else {
        return default_viewport_policy();
    };
    html_viewport_policy(root.node_id, root.attributes, root.children, inherited)
}

fn find_html_node(nodes: &[HtmlDocumentNode]) -> Option<HtmlRootView<'_>> {
    nodes.iter().find_map(|node| {
        let HtmlDocumentNode::Element {
            node_id,
            tag,
            attributes,
            children,
        } = node
        else {
            return None;
        };
        tag.eq_ignore_ascii_case("html").then_some(HtmlRootView {
            node_id: *node_id,
            attributes: attributes.as_slice(),
            children: children.as_slice(),
        })
    })
}

fn html_viewport_policy(
    html_id: u64,
    html_attributes: &[(String, String)],
    html_children: &[HtmlDocumentNode],
    inherited: &CssStyle,
) -> HtmlViewportOverflowPolicy {
    let html_style = CssStyle::from_element("html", html_attributes, inherited);
    if html_style.display == Display::None {
        return default_viewport_policy();
    }

    let html_axes = html_style.computed_overflow_axes();
    if html_axes == (CssOverflowAxis::Visible, CssOverflowAxis::Visible)
        && let Some((body_id, body_axes)) = first_displayed_direct_body(html_children, &html_style)
    {
        return HtmlViewportOverflowPolicy {
            source_node_id: Some(body_id),
            axes: viewport_used_axes(body_axes),
        };
    }

    HtmlViewportOverflowPolicy {
        source_node_id: Some(html_id),
        axes: viewport_used_axes(html_axes),
    }
}

fn default_viewport_policy() -> HtmlViewportOverflowPolicy {
    HtmlViewportOverflowPolicy {
        source_node_id: None,
        axes: DEFAULT_VIEWPORT_OVERFLOW,
    }
}

fn first_displayed_direct_body(
    children: &[HtmlDocumentNode],
    inherited: &CssStyle,
) -> Option<(u64, (CssOverflowAxis, CssOverflowAxis))> {
    children.iter().find_map(|node| {
        let HtmlDocumentNode::Element {
            node_id,
            tag,
            attributes,
            ..
        } = node
        else {
            return None;
        };
        if !tag.eq_ignore_ascii_case("body") {
            return None;
        }
        let style = CssStyle::from_element("body", attributes, inherited);
        (style.display != Display::None).then_some((*node_id, style.computed_overflow_axes()))
    })
}

fn viewport_used_axes(
    axes: (CssOverflowAxis, CssOverflowAxis),
) -> (HtmlBrowserOverflowPolicy, HtmlBrowserOverflowPolicy) {
    (viewport_used_axis(axes.0), viewport_used_axis(axes.1))
}

fn viewport_used_axis(axis: CssOverflowAxis) -> HtmlBrowserOverflowPolicy {
    match axis {
        CssOverflowAxis::Visible => HtmlBrowserOverflowPolicy::Auto,
        CssOverflowAxis::Clip => HtmlBrowserOverflowPolicy::Hidden,
        CssOverflowAxis::Hidden => HtmlBrowserOverflowPolicy::Hidden,
        CssOverflowAxis::Auto => HtmlBrowserOverflowPolicy::Auto,
        CssOverflowAxis::Scroll => HtmlBrowserOverflowPolicy::Scroll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(
        node_id: u64,
        tag: &str,
        style: &str,
        children: Vec<HtmlDocumentNode>,
    ) -> HtmlDocumentNode {
        HtmlDocumentNode::Element {
            node_id,
            tag: tag.to_string(),
            attributes: vec![("style".to_string(), style.to_string())],
            children,
        }
    }

    fn html(children: Vec<HtmlDocumentNode>, style: &str) -> Vec<HtmlDocumentNode> {
        vec![
            HtmlDocumentNode::Text("prefix".to_string()),
            element(1, "html", style, children),
        ]
    }

    #[test]
    fn displayed_direct_body_is_the_source_when_html_is_visible() {
        let nodes = html(
            vec![
                element(2, "div", "overflow: scroll", vec![]),
                element(3, "body", "overflow-x: scroll; overflow-y: clip", vec![]),
            ],
            "overflow: visible",
        );

        assert_eq!(
            viewport_overflow_policy(&nodes, &CssStyle::browser_default()),
            HtmlViewportOverflowPolicy {
                source_node_id: Some(3),
                axes: (
                    HtmlBrowserOverflowPolicy::Scroll,
                    HtmlBrowserOverflowPolicy::Hidden,
                ),
            }
        );
    }

    #[test]
    fn non_visible_html_prevents_body_propagation() {
        let nodes = html(
            vec![element(3, "body", "overflow: scroll", vec![])],
            "overflow-x: clip; overflow-y: auto",
        );

        assert_eq!(
            viewport_overflow_policy(&nodes, &CssStyle::browser_default()),
            HtmlViewportOverflowPolicy {
                source_node_id: Some(1),
                axes: (
                    HtmlBrowserOverflowPolicy::Hidden,
                    HtmlBrowserOverflowPolicy::Auto,
                ),
            }
        );
    }

    #[test]
    fn hidden_or_nested_body_does_not_propagate() {
        let nodes = html(
            vec![
                element(2, "body", "display: none; overflow: scroll", vec![]),
                element(
                    4,
                    "section",
                    "",
                    vec![element(5, "body", "overflow: scroll", vec![])],
                ),
            ],
            "overflow: visible",
        );

        assert_eq!(
            viewport_overflow_policy(&nodes, &CssStyle::browser_default()),
            HtmlViewportOverflowPolicy {
                source_node_id: Some(1),
                axes: DEFAULT_VIEWPORT_OVERFLOW,
            }
        );
    }

    #[test]
    fn missing_or_hidden_html_uses_empty_viewport_policy() {
        assert_eq!(
            viewport_overflow_policy(
                &[HtmlDocumentNode::Text("only text".to_string())],
                &CssStyle::browser_default()
            ),
            HtmlViewportOverflowPolicy {
                source_node_id: None,
                axes: DEFAULT_VIEWPORT_OVERFLOW,
            }
        );
        assert_eq!(
            viewport_overflow_policy(
                &html(
                    vec![element(2, "body", "overflow: scroll", vec![])],
                    "display: none"
                ),
                &CssStyle::browser_default(),
            ),
            HtmlViewportOverflowPolicy {
                source_node_id: None,
                axes: DEFAULT_VIEWPORT_OVERFLOW,
            }
        );
    }
}
