pub(super) fn harfrust_features(settings: Option<&str>) -> Vec<harfrust::Feature> {
    settings
        .into_iter()
        .flat_map(|settings| settings.split(','))
        .filter_map(|setting| {
            let mut parts = setting.split_whitespace();
            let tag = parts.next()?.trim_matches(['\'', '"']);
            let value = parts.next().unwrap_or("1");
            let tag = harfrust::Tag::from_be_bytes(tag.as_bytes().try_into().ok()?);
            let value = value.parse().ok()?;
            Some(harfrust::Feature::new(tag, value, ..))
        })
        .collect()
}
