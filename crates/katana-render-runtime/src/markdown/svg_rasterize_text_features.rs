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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub(crate) enum TextSpacingTrim {
    #[default]
    Normal,
    SpaceAll,
}

#[path = "svg_rasterize_text_han_kerning.rs"]
mod han_kerning;

pub(super) use han_kerning::has_han_candidate;
use han_kerning::{HanKerningTrim, han_kerning_features};
use resvg::usvg::fontdb::{Database, ID};

pub(super) fn features_for_face(
    database: &Database,
    face_id: ID,
    text: &str,
    settings: Option<&str>,
    trim: TextSpacingTrim,
) -> Option<Vec<harfrust::Feature>> {
    let explicit = harfrust_features(settings);
    if !has_han_candidate(text) {
        return Some(explicit);
    }
    let trim = match trim {
        TextSpacingTrim::Normal => HanKerningTrim::Normal,
        TextSpacingTrim::SpaceAll => HanKerningTrim::SpaceAll,
    };
    database.with_face_data(face_id, |bytes, face_index| {
        let synthetic = han_kerning_features(bytes, face_index, text, trim, &explicit).ok()?;
        Some(explicit.into_iter().chain(synthetic).collect())
    })?
}
