use crate::markdown::MarkdownError;

pub(super) struct RegexOps;

impl RegexOps {
    pub(super) fn compile(pattern: &str) -> Result<regex::Regex, MarkdownError> {
        regex::Regex::new(pattern).map_err(|error| MarkdownError::ExportFailed(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::RegexOps;

    #[test]
    fn invalid_regex_pattern_returns_export_error() {
        assert!(RegexOps::compile("(").is_err());
    }
}
