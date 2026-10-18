use serde::Deserialize;
use std::path::PathBuf;

pub(super) struct PlantUmlConfigOps;

impl PlantUmlConfigOps {
    pub(super) fn from_value<T>(
        value: &serde_json::Value,
        validate: impl FnOnce(&T) -> Result<(), String>,
    ) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned + Default,
    {
        if value.is_null() {
            return Ok(T::default());
        }
        let config: T = serde_json::from_value(value.clone()).map_err(|error| error.to_string())?;
        validate(&config)?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub(crate) struct PlantUmlRuntimeConfig {
    #[serde(default, rename = "plantuml_cache_dir", alias = "plantumlCacheDir")]
    cache_dir: String,
}

impl PlantUmlRuntimeConfig {
    pub(crate) fn from_value(value: &serde_json::Value) -> Result<Self, String> {
        PlantUmlConfigOps::from_value(value, Self::validate)
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
        let snake_json = serde_json::json!({"plantuml_cache_dir": "/tmp/krr-cache"});
        let camel_json = serde_json::json!({"plantumlCacheDir": "/tmp/krr-cache"});
        let snake = PlantUmlRuntimeConfig::from_value(&snake_json)?;
        let camel = PlantUmlRuntimeConfig::from_value(&camel_json)?;

        assert_eq!(snake.cache_dir(), camel.cache_dir());
        Ok(())
    }

    #[test]
    fn runtime_config_rejects_control_characters() {
        let result = PlantUmlRuntimeConfig::from_value(&serde_json::json!({
            "plantuml_cache_dir": "/tmp/krr\ncache",
        }));

        assert!(matches!(
            result,
            Err(error) if error.contains("plantuml_cache_dir")
        ));
    }

    #[test]
    fn runtime_config_rejects_non_object_values() {
        let result = PlantUmlRuntimeConfig::from_value(&serde_json::json!("/tmp/krr-cache"));

        assert!(matches!(result, Err(error) if !error.is_empty()));
    }
}
