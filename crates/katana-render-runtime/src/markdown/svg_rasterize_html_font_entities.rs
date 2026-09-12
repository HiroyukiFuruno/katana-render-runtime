pub(super) fn decode_xml_entities(value: &str) -> String {
    let mut decoded = String::with_capacity(value.len());
    let mut remaining = value;
    while let Some(entity_start) = remaining.find('&') {
        decoded.push_str(&remaining[..entity_start]);
        let entity = &remaining[entity_start..];
        let Some(entity_end) = entity.find(';') else {
            decoded.push_str(entity);
            break;
        };
        let candidate = &entity[..=entity_end];
        if let Some(character) = decode_xml_entity(candidate) {
            decoded.push(character);
        } else {
            decoded.push_str(candidate);
        }
        remaining = &entity[entity_end + 1..];
    }
    if !remaining.is_empty() && !remaining.contains('&') {
        decoded.push_str(remaining);
    }
    decoded
}

fn decode_xml_entity(entity: &str) -> Option<char> {
    const DECIMAL_ENTITY_PREFIX_LENGTH: usize = 2;
    const HEX_ENTITY_PREFIX_LENGTH: usize = 3;
    const HEX_RADIX: u32 = 16;
    const ENTITY_SUFFIX_LENGTH: usize = 1;

    match entity {
        "&apos;" => Some('\''),
        "&quot;" => Some('"'),
        "&amp;" => Some('&'),
        "&lt;" => Some('<'),
        "&gt;" => Some('>'),
        _ if entity.starts_with("&#x") || entity.starts_with("&#X") => char::from_u32(
            u32::from_str_radix(
                &entity[HEX_ENTITY_PREFIX_LENGTH..entity.len() - ENTITY_SUFFIX_LENGTH],
                HEX_RADIX,
            )
            .ok()?,
        ),
        _ if entity.starts_with("&#") => char::from_u32(
            entity[DECIMAL_ENTITY_PREFIX_LENGTH..entity.len() - ENTITY_SUFFIX_LENGTH]
                .parse()
                .ok()?,
        ),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::decode_xml_entities;

    #[test]
    fn decodes_named_and_numeric_xml_entities() {
        assert_eq!(
            decode_xml_entities("&apos;DejaVu &amp; &#x53;ans&#39;"),
            "'DejaVu & Sans'"
        );
    }

    #[test]
    fn leaves_malformed_or_unknown_xml_entities_unchanged() {
        assert_eq!(decode_xml_entities("&unknown; &amp"), "&unknown; &amp");
    }
}
