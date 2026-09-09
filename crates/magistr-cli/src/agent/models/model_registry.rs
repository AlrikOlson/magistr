//! Model registry — unified view of all configured models.

use super::builtin_models::builtin_models;
use super::model_entry::ModelEntry;
use super::model_source::ModelSource;
use super::models_file::ModelsFile;
use super::provider::Provider;

/// Model registry — unified view of all configured models.
#[derive(Debug, Clone)]
pub struct ModelRegistry {
    entries: Vec<ModelEntry>,
}

impl ModelRegistry {
    /// Build a registry from built-in defaults only (no user config).
    pub fn defaults() -> Self {
        let entries = builtin_models()
            .into_iter()
            .map(|config| ModelEntry {
                config,
                source: ModelSource::Builtin,
            })
            .collect();
        Self { entries }
    }

    /// Build a registry by merging built-in defaults with a user config file.
    ///
    /// User models with the same `id` as a built-in model override it.
    /// User models with new `id`s are appended.
    pub fn from_config(user_file: &ModelsFile) -> Self {
        let mut entries: Vec<ModelEntry> = builtin_models()
            .into_iter()
            .map(|config| ModelEntry {
                config,
                source: ModelSource::Builtin,
            })
            .collect();

        for user_model in &user_file.models {
            if let Some(existing) = entries.iter_mut().find(|e| e.config.id == user_model.id) {
                // Override built-in with user config
                existing.config = user_model.clone();
                existing.source = ModelSource::UserConfig;
            } else {
                // New model not in builtins
                entries.push(ModelEntry {
                    config: user_model.clone(),
                    source: ModelSource::UserConfig,
                });
            }
        }

        Self { entries }
    }

    /// All model entries.
    pub fn all(&self) -> &[ModelEntry] {
        &self.entries
    }

    /// Look up a model by id.
    pub fn get(&self, id: &str) -> Option<&ModelEntry> {
        self.entries.iter().find(|e| e.config.id == id)
    }

    /// Models that are currently available (API key set or not needed).
    pub fn available(&self) -> Vec<&ModelEntry> {
        self.entries
            .iter()
            .filter(|e| e.config.is_available())
            .collect()
    }

    /// Models that are unavailable (missing API key).
    pub fn unavailable(&self) -> Vec<&ModelEntry> {
        self.entries
            .iter()
            .filter(|e| !e.config.is_available())
            .collect()
    }

    /// Number of models in the registry.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Filter models by provider.
    pub fn by_provider(&self, provider: Provider) -> Vec<&ModelEntry> {
        self.entries
            .iter()
            .filter(|e| e.config.provider == provider)
            .collect()
    }
}
