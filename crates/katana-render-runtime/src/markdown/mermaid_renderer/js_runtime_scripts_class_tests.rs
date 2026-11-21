use super::MermaidRuntimeScripts;
use crate::markdown::diagram_js_runtime::DiagramV8Runtime;

const RAW_CLASS_PATH_COUNT: i32 = 20;

const CLASS_DIAGRAM_10_3: &str = r#"classDiagram
    class PreviewPane {
        +Vec~RenderedSection~ sections
        +full_render(source, path)
        +wait_for_renders()
        +show_content(ui)
    }
    class RenderedSection {
        <<enumeration>>
        Markdown
        Image
        Error
        CommandNotFound
        NotInstalled
        Pending
    }
    PreviewPane --> RenderedSection"#;

const CLASS_DIAGRAM_ENUMERATION_FIXTURE: &str = r#"classDiagram
    class PreviewPane {
        +full_render(source)
        +show_content(ui)
    }
    class RenderedSection {
        <<enumeration>>
        Markdown
        Image
        Error
    }
    PreviewPane --> RenderedSection"#;

#[test]
fn runtime_preserves_raw_enumeration_geometry_across_viewbox_variants() -> Result<(), String> {
    for (view_box, offset) in [
        ("0 0 231.55450000000002 377", 3),
        ("4 4 231.55450000000002 367", 19),
    ] {
        let raw = raw_class_svg(
            &["PreviewPane", "RenderedSection"],
            view_box,
            "231.55450000000002",
            offset,
            true,
        );
        assert_raw_class_geometry_preserved(CLASS_DIAGRAM_ENUMERATION_FIXTURE, &raw)?;
    }
    Ok(())
}

#[test]
fn runtime_preserves_raw_geometry_with_renamed_source_and_dimensions() -> Result<(), String> {
    let source = CLASS_DIAGRAM_ENUMERATION_FIXTURE
        .replace("PreviewPane", "OtherPreview")
        .replace("RenderedSection", "OtherSection");
    let raw = raw_class_svg(
        &["OtherPreview", "OtherSection"],
        "-7 -11 311.5 509.25",
        "311.5",
        43,
        true,
    );
    assert_raw_class_geometry_preserved(&source, &raw)
}

#[test]
fn runtime_preserves_large_enumeration_raw_geometry() -> Result<(), String> {
    let raw = raw_class_svg(
        &[
            "PreviewPane",
            "RenderedSection",
            "CommandNotFound",
            "Pending",
        ],
        "-8 -8 420 620",
        "420",
        71,
        true,
    );
    assert_raw_class_geometry_preserved(CLASS_DIAGRAM_10_3, &raw)
}

#[test]
fn runtime_preserves_raw_inheritance_geometry() -> Result<(), String> {
    for (names, offset) in [
        (vec!["Animal", "Duck", "Fish", "Zebra"], 97),
        (vec!["動物", "アヒル", "魚", "シマウマ"], 113),
    ] {
        let raw = raw_class_svg(&names, "0 0 620.40625 390", "620.40625", offset, false);
        let source = format!(
            "classDiagram\n{} <|-- {}\n{} <|-- {}\n{} <|-- {}",
            names[0], names[1], names[0], names[2], names[0], names[3]
        );
        assert_raw_class_geometry_preserved(&source, &raw)?;
    }
    Ok(())
}

fn assert_raw_class_geometry_preserved(source: &str, raw: &str) -> Result<(), String> {
    let rendered = render_raw_class_svg(source, raw)?;
    /* WHY: 正しい座標表を仮定せず、独立した入力SVGの形状を保持する契約を検査する。 */
    for attribute in ["viewBox", "width", "height", "transform", "d"] {
        assert_eq!(
            attribute_values(&rendered, attribute)?,
            attribute_values(raw, attribute)?,
            "source={source}, attribute={attribute}, rendered={rendered}"
        );
    }
    let max_width = raw
        .split_once("max-width: ")
        .and_then(|(_, rest)| rest.split_once(';'))
        .map(|(value, _)| value)
        .ok_or_else(|| "入力SVGにmax-widthの終端がない".to_string())?;
    assert!(rendered.contains(&format!("max-width: {max_width};")));
    Ok(())
}

fn render_raw_class_svg(source: &str, raw: &str) -> Result<String, String> {
    let request = serde_json::json!({
        "source": source, "svgId": "id", "theme": "dark",
        "background": "#000", "fill": "#111", "text": "#fff",
        "stroke": "#fff", "arrow": "#fff",
    })
    .to_string();
    let serialized = serde_json::to_string(raw).map_err(|error| error.to_string())?;
    let mermaid = format!(
        "globalThis.mermaid = {{initialize() {{}}, render: async () => ({{svg: {serialized}}})}};",
    );
    let scripts = MermaidRuntimeScripts::build_with_zenuml(&mermaid, "", &request);
    DiagramV8Runtime::render(&scripts)
}

fn attribute_values<'a>(svg: &'a str, attribute: &str) -> Result<Vec<&'a str>, String> {
    svg.split(&format!(" {attribute}=\""))
        .skip(1)
        .map(|part| {
            part.split_once('"')
                .map(|(value, _)| value)
                .ok_or_else(|| format!("SVG属性{attribute}の終端引用符がない"))
        })
        .collect()
}

fn raw_class_svg(
    names: &[&str],
    view_box: &str,
    max_width: &str,
    offset: i32,
    enumeration: bool,
) -> String {
    let paths = raw_class_paths(offset);
    let nodes = raw_class_nodes(names, offset);
    let stereotype = if enumeration {
        "<text>«enumeration»</text>"
    } else {
        ""
    };
    format!(
        "<svg id=\"id\" width=\"100%\" height=\"100%\" class=\"classDiagram\" style=\"max-width: {max_width}px;\" viewBox=\"{view_box}\" role=\"graphics-document document\" aria-roledescription=\"class\"><g class=\"root\" transform=\"translate(9, 9)\">{paths}<g class=\"nodes\">{nodes}</g>{stereotype}</g></svg>"
    )
}

fn raw_class_paths(offset: i32) -> String {
    (0..RAW_CLASS_PATH_COUNT)
        .map(|index| {
            format!(
                "<path d=\"M{} {} L{} {}\"></path>",
                offset + index,
                offset + index + 1,
                offset + index + 2,
                offset + index + 3,
            )
        })
        .collect()
}

fn raw_class_nodes(names: &[&str], offset: i32) -> String {
    names.iter().enumerate().map(|(index, name)| {
        format!(
            "<g class=\"node default \" id=\"classId-{name}-{index}\" transform=\"translate({}, {})\"><g transform=\"translate({}, {})\"><text>{name}</text><g class=\"methods-group text\" transform=\"translate(1, 2)\"><text>+method()</text></g></g></g>",
            offset + index as i32 * 7, offset + index as i32 * 11, offset + 5, offset - 9,
        )
    }).collect()
}
