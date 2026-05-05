pub(super) struct HtmlFileUri;

impl HtmlFileUri {
    pub(super) fn from_path(path: &std::path::Path) -> String {
        let normalized_path = path.to_string_lossy().replace('\\', "/");
        let encoded_path = Self::encode_path(&normalized_path);
        if Self::has_windows_drive_prefix(&encoded_path) {
            return format!("file:///{encoded_path}");
        }
        format!("file://{encoded_path}")
    }

    fn has_windows_drive_prefix(path: &str) -> bool {
        let bytes = path.as_bytes();
        bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
    }

    fn encode_path(path: &str) -> String {
        let mut encoded = String::with_capacity(path.len());
        for byte in path.as_bytes() {
            if Self::is_path_byte(*byte) {
                encoded.push(*byte as char);
            } else {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
        encoded
    }

    fn is_path_byte(byte: u8) -> bool {
        matches!(
            byte,
            b'A'..=b'Z'
                | b'a'..=b'z'
                | b'0'..=b'9'
                | b'-'
                | b'.'
                | b'_'
                | b'~'
                | b'/'
                | b':'
        )
    }
}

#[cfg(test)]
#[path = "html_uri_tests.rs"]
mod tests;
