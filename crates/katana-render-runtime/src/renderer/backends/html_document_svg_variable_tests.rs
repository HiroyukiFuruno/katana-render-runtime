use super::quoted_style_tests::{
    assert_svg_style_contains, with_projected_markup, with_svg_element,
};

#[test]
fn svg_without_variable_inputs_keeps_source_attributes_for_the_svg_renderer() -> Result<(), String>
{
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><style>.plain { fill: red }</style><rect id="plain" class="plain" width="12" fill="#123456"/></svg>"##;
    with_projected_markup(svg, |markup| {
        with_svg_element(markup, "plain", |element| {
            assert_eq!(
                element.attributes.get("fill").map(String::as_str),
                Some("#123456")
            );
            assert_eq!(
                element.attributes.get("width").map(String::as_str),
                Some("12")
            );
            assert_eq!(
                element.attributes.get("class").map(String::as_str),
                Some("plain")
            );
            assert!(!element.attributes.contains_key("style"));
            Ok(())
        })?;
        Ok(())
    })?;
    Ok(())
}

#[test]
fn svg_projection_resolves_variable_fallback_and_keeps_variable_free_attributes()
-> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg"><rect id="fallback" width="12" fill="#111111" style="fill: var(--missing, #ffffff); stroke: blue"/><rect id="plain" width="12" fill="#123456"/></svg>"##;
    with_projected_markup(svg, |markup| {
        for (id, fill, style) in [
            (
                "fallback",
                "#111111",
                Some("fill: var(--missing, #ffffff); stroke: blue; fill: #ffffff"),
            ),
            ("plain", "#123456", None),
        ] {
            with_svg_element(markup, id, |element| {
                assert_eq!(
                    element.attributes.get("fill").map(String::as_str),
                    Some(fill)
                );
                assert_eq!(
                    element.attributes.get("width").map(String::as_str),
                    Some("12")
                );
                assert_eq!(element.attributes.get("style").map(String::as_str), style);
                Ok(())
            })?;
        }
        Ok(())
    })?;
    Ok(())
}

#[test]
fn svg_projection_resolves_inherited_and_overridden_custom_properties() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" style="--tone: #336699"><rect id="root-inherited" style="fill: var(--tone)"/><g id="override" style="--tone: #cc3300"><rect id="inherited" style="fill: var(--tone)"/><rect id="child" style="--tone: #228844; fill: var(--tone)"/></g></svg>"##;
    with_projected_markup(svg, |markup| {
        for (id, expected) in [
            ("root-inherited", "fill: #336699"),
            ("inherited", "fill: #cc3300"),
            ("child", "fill: #228844"),
        ] {
            assert_svg_style_contains(markup, id, expected)?;
        }
        Ok(())
    })?;
    Ok(())
}

#[test]
fn svg_projection_obeys_specificity_and_important_for_variable_rules() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" style="--tone: #123456"><style>.shape { fill: var(--tone, #00ff00) !important } #target { fill: var(--missing, #ff0000) !important }</style><rect id="target" class="shape" fill="#111111"/></svg>"##;
    with_projected_markup(svg, |markup| {
        assert_svg_style_contains(markup, "target", "fill: #ff0000 !important")?;
        with_svg_element(markup, "target", |element| {
            assert_eq!(
                element.attributes.get("fill").map(String::as_str),
                Some("#111111")
            );
            Ok(())
        })?;
        Ok(())
    })?;
    Ok(())
}

#[test]
fn svg_projection_uses_existing_cycle_fallback_and_unresolved_behavior() -> Result<(), String> {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" style="--cycle: var(--cycle)"><style>.cycled { stroke: var(--cycle, #abcdef) } .unresolved { opacity: var(--cycle) }</style><rect id="cycle" class="cycled"/><rect id="unresolved" class="unresolved"/></svg>"##;
    with_projected_markup(svg, |markup| {
        assert_svg_style_contains(markup, "cycle", "stroke: #abcdef")?;
        with_svg_element(markup, "unresolved", |element| {
            assert!(
                element
                    .attributes
                    .get("style")
                    .is_none_or(|style| !style.contains("opacity:"))
            );
            Ok(())
        })?;
        Ok(())
    })?;
    Ok(())
}
