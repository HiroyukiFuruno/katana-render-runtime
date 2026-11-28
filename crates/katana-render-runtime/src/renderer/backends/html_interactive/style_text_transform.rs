use super::{CssStyle, CssTextTransform};
use std::borrow::Cow;

impl CssStyle {
    pub(in crate::renderer::backends::html_interactive) fn transformed_text<'a>(
        &self,
        text: &'a str,
    ) -> Cow<'a, str> {
        match self.text_transform {
            CssTextTransform::None => Cow::Borrowed(text),
            CssTextTransform::Uppercase => Cow::Owned(text.to_uppercase()),
            CssTextTransform::Lowercase => Cow::Owned(text.to_lowercase()),
            CssTextTransform::Capitalize => {
                let mut word_start = true;
                let mut transformed = String::with_capacity(text.len());
                for character in text.chars() {
                    if word_start && character.is_alphabetic() {
                        transformed.extend(character.to_uppercase());
                    } else {
                        transformed.push(character);
                    }
                    word_start = !character.is_alphanumeric();
                }
                Cow::Owned(transformed)
            }
        }
    }
}
