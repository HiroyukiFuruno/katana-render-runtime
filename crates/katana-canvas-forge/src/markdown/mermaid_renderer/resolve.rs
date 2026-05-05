use std::path::PathBuf;

pub struct MermaidBinaryOps;

impl MermaidBinaryOps {
    pub fn default_install_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".local").join("katana").join("mermaid.min.js"))
    }

    pub fn resolve_mermaid_js() -> Result<PathBuf, String> {
        Self::resolve_mermaid_js_with_env(
            std::env::var_os("MERMAID_JS"),
            Self::default_install_path(),
        )
    }

    fn resolve_mermaid_js_with_env(
        env_value: Option<std::ffi::OsString>,
        home_path: Option<PathBuf>,
    ) -> Result<PathBuf, String> {
        if let Some(path) = Self::env_mermaid_js_from(env_value)? {
            return Ok(path);
        }

        Self::resolve_mermaid_js_with_home(home_path)
    }

    fn env_mermaid_js_from(value: Option<std::ffi::OsString>) -> Result<Option<PathBuf>, String> {
        let Some(path) = value else {
            return Ok(None);
        };
        if path.is_empty() {
            return Err("MERMAID_JS is empty".to_string());
        }
        Ok(Some(PathBuf::from(path)))
    }

    fn resolve_mermaid_js_with_home(home_path: Option<PathBuf>) -> Result<PathBuf, String> {
        home_path
            .ok_or_else(|| "home directory is unavailable for Mermaid.js resolution".to_string())
    }

    pub fn find_mermaid_js() -> Result<Option<PathBuf>, String> {
        Self::find_mermaid_js_from(Self::resolve_mermaid_js())
    }

    fn find_mermaid_js_from(path: Result<PathBuf, String>) -> Result<Option<PathBuf>, String> {
        let path = path?;
        Ok(path.exists().then_some(path))
    }
}

#[cfg(test)]
mod tests {
    use super::MermaidBinaryOps;

    #[test]
    fn resolve_mermaid_js_reports_missing_home_without_fallback() {
        let result = MermaidBinaryOps::resolve_mermaid_js_with_home(None);

        assert!(matches!(result, Err(error) if error.contains("home directory")));
    }

    #[test]
    fn resolve_mermaid_js_accepts_explicit_env_override() {
        let result = MermaidBinaryOps::resolve_mermaid_js_with_env(
            Some(std::ffi::OsString::from("runtime.js")),
            None,
        );

        assert!(matches!(result, Ok(path) if path == std::path::Path::new("runtime.js")));
    }

    #[test]
    fn resolve_mermaid_js_rejects_empty_env_override() {
        let result = MermaidBinaryOps::resolve_mermaid_js_with_env(
            Some(std::ffi::OsString::new()),
            Some(std::path::PathBuf::from("fallback.js")),
        );

        assert!(matches!(result, Err(error) if error.contains("MERMAID_JS")));
    }

    #[test]
    fn find_mermaid_js_propagates_resolution_errors() {
        let result = MermaidBinaryOps::find_mermaid_js_from(Err("boom".to_string()));

        assert!(matches!(result, Err(error) if error == "boom"));
    }

    #[test]
    fn env_mermaid_js_rejects_empty_override() {
        let result = MermaidBinaryOps::env_mermaid_js_from(Some(std::ffi::OsString::new()));

        assert!(matches!(result, Err(error) if error.contains("MERMAID_JS")));
    }

    #[test]
    fn env_mermaid_js_treats_missing_override_as_optional() {
        let result = MermaidBinaryOps::env_mermaid_js_from(None);

        assert!(matches!(result, Ok(None)));
    }
}
