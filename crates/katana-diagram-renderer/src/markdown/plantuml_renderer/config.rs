use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct PlantUmlRuntimeConfig {
    #[serde(default, rename = "plantuml_cache_dir", alias = "plantumlCacheDir")]
    cache_dir: String,
}

impl PlantUmlRuntimeConfig {
    pub(crate) fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        if value.is_null() {
            return Ok(Self::default());
        }
        let config: Self =
            serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        config.validate()?;
        Ok(config)
    }

    pub(crate) fn cache_dir(&self) -> Option<PathBuf> {
        let value = self.cache_dir.trim();
        (!value.is_empty()).then(|| PathBuf::from(value))
    }

    fn validate(&self) -> Result<(), String> {
        if self.cache_dir.chars().any(char::is_control) {
            return Err("plantuml_cache_dir must not contain control characters".to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PlantUmlRuntimeConfig;

    #[test]
    fn runtime_config_accepts_snake_and_camel_cache_dir() -> Result<(), String> {
        let snake = PlantUmlRuntimeConfig::from_value(&serde_json::json!({
            "plantuml_cache_dir": "/tmp/kdr-cache",
        }))?;
        let camel = PlantUmlRuntimeConfig::from_value(&serde_json::json!({
            "plantumlCacheDir": "/tmp/kdr-cache",
        }))?;

        assert_eq!(snake.cache_dir(), camel.cache_dir());
        Ok(())
    }

    #[test]
    fn runtime_config_rejects_control_characters() {
        let result = PlantUmlRuntimeConfig::from_value(&serde_json::json!({
            "plantuml_cache_dir": "/tmp/kdr\ncache",
        }));

        assert!(matches!(
            result,
            Err(error) if error.contains("plantuml_cache_dir")
        ));
    }
}
