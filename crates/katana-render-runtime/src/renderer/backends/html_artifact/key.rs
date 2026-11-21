use super::types::HtmlArtifactError;

const MAX_KEY_LENGTH: usize = 128;

pub(super) fn validate_key(key: &str) -> Result<(), HtmlArtifactError> {
    if key.is_empty()
        || key.len() > MAX_KEY_LENGTH
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        || is_windows_reserved_key(key)
    {
        return Err(HtmlArtifactError::InvalidKey);
    }
    Ok(())
}

fn is_windows_reserved_key(key: &str) -> bool {
    let upper = key.to_ascii_uppercase();
    matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    )
}

#[cfg(test)]
mod tests {
    use super::validate_key;

    #[test]
    fn artifact_keys_are_portable_file_names() {
        assert!(validate_key("frame-01_A").is_ok());
        for key in [
            "",
            ".",
            "..",
            "frame.json",
            "frame:1",
            "frame/1",
            "frame\\1",
        ] {
            assert!(
                validate_key(key).is_err(),
                "key should be rejected: {key:?}"
            );
        }
        for key in ["CON", "con", "PRN", "AUX", "NUL", "COM1", "LPT9"] {
            assert!(
                validate_key(key).is_err(),
                "reserved key should be rejected: {key}"
            );
        }
    }
}
