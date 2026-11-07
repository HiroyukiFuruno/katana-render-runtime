use unicode_linebreak::{BreakClass, break_property, linebreaks};

pub(in super::super) fn css_auto_linebreak_ends(text: &str) -> Vec<usize> {
    if !text.chars().any(is_conditional_japanese_starter) {
        return suppress_slash_letter_breaks(text, linebreaks(text).map(|(end, _)| end).collect());
    }
    /* WHY: Chromeのauto改行ではCJをIDとして扱い、他の禁則・結合制御は維持する。 */
    let mut normalized = String::with_capacity(text.len());
    let mut offsets = Vec::new();
    for (start, character) in text.char_indices() {
        normalized.push(if is_conditional_japanese_starter(character) {
            '一'
        } else {
            character
        });
        offsets.push((normalized.len(), start + character.len_utf8()));
    }
    let ends = linebreaks(&normalized)
        .filter_map(|(end, _)| {
            offsets
                .binary_search_by_key(&end, |(offset, _)| *offset)
                .ok()
                .map(|index| offsets[index].1)
        })
        .collect();
    suppress_slash_letter_breaks(text, ends)
}

pub(super) fn suppress_slash_letter_breaks(text: &str, ends: Vec<usize>) -> Vec<usize> {
    ends.into_iter()
        .filter(|&end| {
            let Some('/') = text[..end].chars().next_back() else {
                return true;
            };
            !text[end..].chars().next().is_some_and(is_css_letter)
        })
        .collect()
}

/* CSS Textのslash/letter規則はGeneral Category Lを対象にする。直接の
 * category table依存がないため、現状はASCII letterを保守的に扱う。 */
pub(super) fn is_css_letter(character: char) -> bool {
    character.is_ascii_alphabetic()
}

fn is_conditional_japanese_starter(character: char) -> bool {
    break_property(character as u32) == BreakClass::ConditionalJapaneseStarter
}
