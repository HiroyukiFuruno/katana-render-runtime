use resvg::usvg::roxmltree::Document;

use super::geometry_contract::GeometryContract;
use super::{C4_DYNAMIC_SOURCE, render_mermaid_svg};

#[test]
fn actual_mermaid_c4_rejects_hidden_root_geometry() -> Result<(), String> {
    let actual = render_mermaid_svg(C4_DYNAMIC_SOURCE);
    let actual = actual.as_ref().map_err(|error| error.to_owned())?;
    GeometryContract::dynamic(actual, false)?;
    let parsed = Document::parse(actual).map_err(|error| error.to_string())?;
    if parsed.root_element().attribute("opacity").is_some() {
        return Err("actual C4 root already declares opacity".into());
    }

    let root_start = actual
        .find("<svg")
        .ok_or("actual Mermaid SVG root missing")?;
    let mut hidden = String::from(actual.as_str());
    hidden.insert_str(root_start + "<svg".len(), " opacity=\"0\"");
    let error = GeometryContract::dynamic(&hidden, false)
        .err()
        .ok_or_else(|| "hidden root must reject semantic SVG geometry".to_owned())?;
    assert_eq!(error, "semantic geometry is hidden");
    Ok(())
}
