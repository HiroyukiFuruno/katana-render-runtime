use crate::markdown::color_preset::DiagramColorPreset;
use serde::Deserialize;

pub(crate) struct PlantUmlThemeOps;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct PlantUmlThemeConfig {
    #[serde(
        default,
        rename = "plantuml_theme",
        alias = "plantumlTheme",
        alias = "theme"
    )]
    theme: String,
    #[serde(
        default,
        rename = "plantuml_theme_from",
        alias = "plantumlThemeFrom",
        alias = "themeFrom"
    )]
    theme_from: String,
    #[serde(
        default,
        rename = "plantuml_theme_mode",
        alias = "plantumlThemeMode",
        alias = "themeMode"
    )]
    theme_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlantUmlRenderStyle {
    dark_mode: bool,
    config_lines: Vec<String>,
}

impl PlantUmlThemeOps {
    pub(crate) fn style(
        preset: &DiagramColorPreset,
        config: &PlantUmlThemeConfig,
    ) -> PlantUmlRenderStyle {
        PlantUmlRenderStyle::new(
            config.dark_mode(preset.dark_mode),
            Self::config_lines(config),
        )
    }

    fn config_lines(config: &PlantUmlThemeConfig) -> Vec<String> {
        config
            .theme_directive()
            .map_or_else(Vec::new, |directive| vec![directive])
    }
}

impl PlantUmlThemeConfig {
    pub(crate) fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let config: Self =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), String> {
        if self.theme.trim().is_empty() && !self.theme_from.trim().is_empty() {
            return Err("plantuml_theme_from requires plantuml_theme".to_string());
        }
        Self::validate_theme_name(&self.theme)?;
        Self::validate_theme_from(&self.theme_from)?;
        self.validate_dark_mode()?;
        Ok(())
    }

    fn validate_theme_name(theme: &str) -> Result<(), String> {
        if theme
            .chars()
            .any(|it| it.is_whitespace() || it.is_control())
        {
            return Err("plantuml_theme must not contain whitespace or control characters".into());
        }
        Ok(())
    }

    fn validate_theme_from(theme_from: &str) -> Result<(), String> {
        if theme_from.chars().any(char::is_control) {
            return Err("plantuml_theme_from must not contain control characters".into());
        }
        Ok(())
    }

    fn theme_directive(&self) -> Option<String> {
        let theme = self.theme.trim();
        if theme.is_empty() {
            return None;
        }
        let theme_from = self.theme_from.trim();
        if theme_from.is_empty() {
            return Some(format!("!theme {theme}"));
        }
        Some(format!("!theme {theme} from {theme_from}"))
    }

    fn dark_mode(&self, fallback: bool) -> bool {
        match self.theme_mode.trim() {
            "" => fallback,
            "dark" => true,
            "light" => false,
            _ => unreachable!("PlantUML theme mode must be validated before rendering"),
        }
    }

    fn validate_dark_mode(&self) -> Result<(), String> {
        match self.theme_mode.trim() {
            "" | "dark" | "light" => Ok(()),
            _ => Err("plantuml_theme_mode must be dark or light".to_string()),
        }
    }
}

impl PlantUmlRenderStyle {
    fn new(dark_mode: bool, config_lines: Vec<String>) -> Self {
        Self {
            dark_mode,
            config_lines,
        }
    }

    pub(crate) fn dark_mode(&self) -> bool {
        self.dark_mode
    }

    pub(crate) fn config_lines(&self) -> &[String] {
        &self.config_lines
    }
}

#[cfg(test)]
mod tests {
    use super::{PlantUmlThemeConfig, PlantUmlThemeOps};
    use crate::markdown::color_preset::DiagramColorPreset;

    #[test]
    fn dark_preset_uses_official_dark_mode_without_skinparam() {
        let config = PlantUmlThemeConfig::default();
        let style = PlantUmlThemeOps::style(DiagramColorPreset::dark(), &config);

        assert!(style.dark_mode());
        assert!(style.config_lines().is_empty());
    }

    #[test]
    fn plantuml_theme_config_builds_official_theme_directive() -> Result<(), String> {
        let config = PlantUmlThemeConfig::from_value(&serde_json::json!({
            "plantumlTheme": "cyborg",
            "plantumlThemeFrom": "/path/to/themes",
            "plantumlThemeMode": "light",
        }))?;
        let style = PlantUmlThemeOps::style(DiagramColorPreset::light(), &config);

        assert!(!style.dark_mode());
        assert_eq!(
            style.config_lines(),
            &[String::from("!theme cyborg from /path/to/themes")]
        );
        Ok(())
    }

    #[test]
    fn theme_mode_overrides_preset_dark_mode() -> Result<(), String> {
        let config = PlantUmlThemeConfig::from_value(&serde_json::json!({
            "plantuml_theme_mode": "light",
        }))?;
        let style = PlantUmlThemeOps::style(DiagramColorPreset::dark(), &config);

        assert!(!style.dark_mode());
        Ok(())
    }

    #[test]
    fn plantuml_theme_config_rejects_injection_like_values() {
        let result = PlantUmlThemeConfig::from_value(&serde_json::json!({
            "plantumlTheme": "cyborg\nskinparam monochrome true",
        }));

        assert!(result.is_err());
    }

    #[test]
    fn plantuml_theme_config_rejects_invalid_theme_mode() {
        let result = PlantUmlThemeConfig::from_value(&serde_json::json!({
            "plantuml_theme_mode": "sepia",
        }));

        assert!(result.is_err());
    }
}
