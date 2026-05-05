use std::path::PathBuf;

pub struct MermaidBinaryOps;

impl MermaidBinaryOps {
    pub fn default_install_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".local").join("katana").join("mermaid.min.js"))
    }

    pub fn resolve_mermaid_js() -> Result<PathBuf, String> {
        /* WHY: environment override is useful in CI and local debugging scenarios. */
        if let Ok(path) = std::env::var("MERMAID_JS") {
            return Ok(PathBuf::from(path));
        }

        Self::resolve_mermaid_js_with_home(Self::default_install_path())
    }

    fn resolve_mermaid_js_with_home(home_path: Option<PathBuf>) -> Result<PathBuf, String> {
        home_path
            .ok_or_else(|| "home directory is unavailable for Mermaid.js resolution".to_string())
    }

    pub fn find_mermaid_js() -> Result<Option<PathBuf>, String> {
        let path = Self::resolve_mermaid_js()?;
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
}
