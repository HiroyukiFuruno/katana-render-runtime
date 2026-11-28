use super::MermaidRenderOps;
use std::sync::atomic::{AtomicU64, Ordering};

static MERMAID_SVG_RENDER_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl MermaidRenderOps {
    pub(super) fn unique_svg_instance(svg: String) -> String {
        let Some(root_id) = Self::root_svg_id(&svg) else {
            return svg;
        };
        let sequence = MERMAID_SVG_RENDER_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let unique_id = format!("{root_id}-{sequence:016x}");
        svg.replace(&root_id, &unique_id)
    }

    fn root_svg_id(svg: &str) -> Option<String> {
        let svg_start = svg.find("<svg")?;
        let open_end = svg_start + svg[svg_start..].find('>')?;
        let marker = r#"id="katana-mermaid-svg-"#;
        let start = svg_start + svg[svg_start..open_end].find(marker)? + r#"id=""#.len();
        let end = start + svg[start..open_end].find('"')?;
        Some(svg[start..end].to_string())
    }
}
