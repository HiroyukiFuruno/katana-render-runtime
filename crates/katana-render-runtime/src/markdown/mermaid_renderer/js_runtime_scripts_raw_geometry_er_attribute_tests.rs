use super::geometry_tree::raw_geometry_tree;
use xml::reader::XmlEvent;

type ErLabelTransform<'a> = (&'a str, f64, f64);

pub(super) fn assert_er_attribute_layout(svg: &str) -> Result<(), String> {
    let tree = raw_geometry_tree(svg)?;
    let labels = tree
        .0
        .iter()
        .filter_map(parse_er_label)
        .collect::<Result<Vec<_>, _>>()?;
    assert_er_attribute_groups(&labels)
}

fn parse_er_label(event: &XmlEvent) -> Option<Result<ErLabelTransform<'_>, String>> {
    let XmlEvent::StartElement {
        name, attributes, ..
    } = event
    else {
        return None;
    };
    if name.local_name != "g" {
        return None;
    }
    let class = attributes
        .iter()
        .find(|attribute| attribute.name.local_name == "class")
        .map(|attribute| attribute.value.as_str())?;
    if !class
        .split_whitespace()
        .any(|token| matches!(token, "attribute-type" | "attribute-name"))
    {
        return None;
    }
    Some(parse_er_transform(class, attributes))
}

fn parse_er_transform<'a>(
    class: &'a str,
    attributes: &[xml::attribute::OwnedAttribute],
) -> Result<ErLabelTransform<'a>, String> {
    let transform = attributes
        .iter()
        .find(|attribute| attribute.name.local_name == "transform")
        .map(|attribute| attribute.value.as_str())
        .ok_or_else(|| format!("ER attribute label lacks transform: {class}"))?;
    let values = transform
        .strip_prefix("translate(")
        .and_then(|value| value.strip_suffix(')'))
        .ok_or_else(|| format!("ER attribute label has unsupported transform: {transform}"))?;
    let mut parts = values.split(',').map(str::trim);
    let x = parse_er_axis(&mut parts, "x", transform)?;
    let y = parse_er_axis(&mut parts, "y", transform)?;
    if !x.is_finite() || !y.is_finite() || parts.next().is_some() {
        return Err(format!(
            "ER attribute label transform is invalid: {transform}"
        ));
    }
    Ok((class, x, y))
}

fn parse_er_axis<'a>(
    parts: &mut impl Iterator<Item = &'a str>,
    axis: &str,
    transform: &str,
) -> Result<f64, String> {
    parts
        .next()
        .ok_or_else(|| format!("ER attribute label has no {axis}: {transform}"))?
        .parse::<f64>()
        .map_err(|error| format!("ER attribute label {axis} is invalid: {error}"))
}

fn assert_er_attribute_groups(labels: &[ErLabelTransform<'_>]) -> Result<(), String> {
    let types = labels
        .iter()
        .filter(|(class, _, _)| {
            class
                .split_whitespace()
                .any(|token| token == "attribute-type")
        })
        .collect::<Vec<_>>();
    let names = labels
        .iter()
        .filter(|(class, _, _)| {
            class
                .split_whitespace()
                .any(|token| token == "attribute-name")
        })
        .collect::<Vec<_>>();
    assert_er_attribute_group_counts(types.len(), names.len())?;
    assert_er_attribute_group_alignment(&types, &names)
}

fn assert_er_attribute_group_counts(types: usize, names: usize) -> Result<(), String> {
    if types == 0 || types != names {
        return Err(format!(
            "ER attribute label groups are incomplete: types={types}, names={names}"
        ));
    }
    Ok(())
}

fn assert_er_attribute_group_alignment(
    types: &[&(&str, f64, f64)],
    names: &[&(&str, f64, f64)],
) -> Result<(), String> {
    if types
        .iter()
        .zip(names.iter())
        .any(|((_, type_x, type_y), (_, name_x, name_y))| {
            (*type_x - *name_x).abs() < 1.0 || (*type_y - *name_y).abs() > 1.0
        })
    {
        return Err("ER attribute columns overlap or rows are misaligned".to_string());
    }
    Ok(())
}
