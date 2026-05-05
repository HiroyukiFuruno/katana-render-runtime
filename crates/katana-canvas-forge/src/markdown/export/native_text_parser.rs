pub(super) use super::native_text_parser_code::extract_code_blocks;
pub(super) use super::native_text_parser_html::{
    block_tags_to_breaks, body_content, decode_entities, mark_headings, remove_tag_blocks,
    replace_image_alt, strip_tags,
};
pub(super) use super::native_text_parser_lines::parse_typed_lines;
