use crate::renderer::backends::html_document::HtmlDocument;
use crate::renderer::backends::html_subresources::HtmlDocumentResources;
use markup5ever_rcdom::{Handle, NodeData};

pub(super) fn requires_javascript(
    document: &HtmlDocument,
    resources: &HtmlDocumentResources,
) -> bool {
    !resources.scripts.is_empty()
        || resources.body_onload_source.is_some()
        || document.body_onload_source.is_some()
        || !document.later_body_onload_tokens.is_empty()
        || node_requires_javascript(&document.document)
}

fn node_requires_javascript(node: &Handle) -> bool {
    if let NodeData::Element { name, attrs, .. } = &node.data {
        /* WHY: 埋込み文書や遅延展開の内容は、このDOM走査だけでは実行経路を否定できない。 */
        if matches!(
            name.local.as_ref(),
            "script" | "iframe" | "frame" | "frameset" | "object" | "embed" | "template"
        ) || attrs.borrow().iter().any(|attribute| {
            let name = attribute.name.local.as_str();
            name.get(..2)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("on"))
                || name.eq_ignore_ascii_case("srcdoc")
                || is_javascript_url(attribute.value.as_ref())
        }) {
            return true;
        }
    }
    node.children.borrow().iter().any(node_requires_javascript)
}

fn is_javascript_url(value: &str) -> bool {
    /* WHY: URL解析は先頭末尾のC0制御文字と空白を除き、途中のTAB/CR/LFも取り除く。 */
    let mut bytes = value
        .trim_matches(|character: char| character <= '\u{20}')
        .bytes()
        .filter(|byte| !matches!(byte, b'\t' | b'\r' | b'\n'));
    b"javascript:".iter().all(|expected| {
        bytes
            .next()
            .is_some_and(|byte| byte.eq_ignore_ascii_case(expected))
    })
}

#[cfg(test)]
mod tests {
    use super::requires_javascript;
    use crate::renderer::backends::html_document::{HtmlDocument, HtmlDocumentScript};
    use crate::renderer::backends::html_subresources::HtmlDocumentResources;
    use std::collections::HashMap;

    fn resources() -> HtmlDocumentResources {
        HtmlDocumentResources {
            stylesheets: HashMap::new(),
            scripts: Vec::new(),
            body_onload_script_index: None,
            body_onload_source: None,
        }
    }

    #[test]
    fn plain_layout_and_native_controls_do_not_require_javascript() {
        for html in [
            "<main><style>.flex{display:flex}</style><section class=flex><p>Layout</p></section></main>",
            "<a href='#end'>Jump</a><input><details><summary>Open</summary></details>",
            "<a href='https://example.test/javascript:note'>Link</a>",
        ] {
            assert!(!requires_javascript(
                &HtmlDocument::parse(html),
                &resources()
            ));
        }
    }

    #[test]
    fn executable_attributes_and_embedded_documents_keep_runtime() {
        for html in [
            "<button onclick=''>Click</button>",
            "<body ONLOAD='void 0'><img onerror='void 0'>",
            "<a href=' &#9;JaVa&#10;ScRi&#13;Pt:alert(1)'>Link</a>",
            "<svg><a xlink:href='javascript:alert(1)'>Link</a></svg>",
            "<form action='javascript:alert(1)'></form>",
            "<iframe src='data:text/html,hello'></iframe>",
            "<object data='nested.html'></object><embed src='nested.svg'>",
            "<template><button onclick='alert(1)'>Click</button></template>",
            "<script type='application/json'>{}</script>",
        ] {
            assert!(requires_javascript(
                &HtmlDocument::parse(html),
                &resources()
            ));
        }
    }

    #[test]
    fn collected_scripts_and_detached_lifecycle_sources_keep_runtime() {
        let document = HtmlDocument::parse("<p>Plain</p>");
        let mut loaded = resources();
        loaded
            .scripts
            .push(HtmlDocumentScript::Source(String::new()));
        assert!(requires_javascript(&document, &loaded));
        loaded.scripts.clear();
        loaded.body_onload_source = Some(String::new());
        assert!(requires_javascript(&document, &loaded));
    }
}
