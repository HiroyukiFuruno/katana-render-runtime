use core_graphics::{data_provider::CGDataProvider, font::CGFont};
use std::sync::Arc;

pub(super) fn create_typeface(
    source_bytes: &[u8],
    expected_index: u32,
) -> Result<Option<CGFont>, String> {
    let bytes = if expected_index == 0
        && rustybuzz::ttf_parser::fonts_in_collection(source_bytes).is_none()
    {
        source_bytes.to_vec()
    } else {
        super::ttc::repack_face(source_bytes, expected_index)?
    };
    let provider = CGDataProvider::from_buffer(Arc::new(bytes));
    CGFont::from_data_provider(provider)
        .map(Some)
        .map_err(|_| "CoreText physical font creation failed".to_string())
}
