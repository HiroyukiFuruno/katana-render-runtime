pub(super) fn has_attribute(attributes: &[(String, String)], name: &str) -> bool {
    attribute(attributes, name).is_some()
}

pub(super) fn attribute<'a>(attributes: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attributes
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

pub(super) fn style_value<'a>(
    attributes: &'a [(String, String)],
    property: &str,
) -> Option<&'a str> {
    attribute(attributes, "style")?
        .split(';')
        .find_map(|declaration| {
            let (name, value) = declaration.split_once(':')?;
            name.trim()
                .eq_ignore_ascii_case(property)
                .then_some(value.split("!important").next()?.trim())
        })
}
