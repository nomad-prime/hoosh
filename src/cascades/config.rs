use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::cascades::{CascadeError, ExecutionTier};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RoutingPolicy {
    #[default]
    Conservative,
    Aggressive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    pub model: String,
    #[serde(default)]
    pub parameters: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CascadeConfig {
    pub enabled: bool,
    #[serde(default)]
    pub routing_policy: RoutingPolicy,
    #[serde(default)]
    pub default_tier: Option<String>,
    pub models: HashMap<String, TierConfig>,
    #[serde(default = "default_confidence_threshold")]
    pub confidence_threshold: f32,
}

fn default_confidence_threshold() -> f32 {
    0.7
}

impl CascadeConfig {
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            routing_policy: RoutingPolicy::Conservative,
            default_tier: None,
            models: HashMap::new(),
            confidence_threshold: 0.7,
        }
    }

    pub fn validate(&self) -> Result<(), CascadeError> {
        if !self.enabled {
            return Ok(());
        }

        if self.models.is_empty() {
            return Err(CascadeError::InvalidConfig(
                "Cascades enabled but no models configured".to_string(),
            ));
        }

        let required_tiers = ["light", "medium", "heavy"];
        for tier in required_tiers.iter() {
            if !self.models.contains_key(*tier) {
                return Err(CascadeError::InvalidConfig(format!(
                    "Missing required tier configuration: {}",
                    tier
                )));
            }
        }

        if self.confidence_threshold < 0.0 || self.confidence_threshold > 1.0 {
            return Err(CascadeError::InvalidConfig(
                "Confidence threshold must be between 0.0 and 1.0".to_string(),
            ));
        }

        Ok(())
    }

    pub fn get_model_for_tier(&self, tier: ExecutionTier) -> Result<String, CascadeError> {
        let tier_name = match tier {
            ExecutionTier::Light => "light",
            ExecutionTier::Medium => "medium",
            ExecutionTier::Heavy => "heavy",
        };

        self.models
            .get(tier_name)
            .map(|tc| tc.model.clone())
            .ok_or_else(|| CascadeError::ModelTierNotFound(tier_name.to_string()))
    }

    pub fn tier_names(&self) -> Vec<&str> {
        self.models.keys().map(|k| k.as_str()).collect()
    }
}

impl Default for CascadeConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_disabled_config_is_valid() {
        let config = CascadeConfig::disabled();
        assert!(!config.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_enabled_config_without_models_fails() {
        let config = CascadeConfig {
            enabled: true,
            routing_policy: RoutingPolicy::Conservative,
            default_tier: None,
            models: HashMap::new(),
            confidence_threshold: 0.7,
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_enabled_config_with_all_tiers_succeeds() {
        let mut models = HashMap::new();
        models.insert(
            "light".to_string(),
            TierConfig {
                model: "gpt-3.5-turbo".to_string(),
                parameters: HashMap::new(),
            },
        );
        models.insert(
            "medium".to_string(),
            TierConfig {
                model: "gpt-4".to_string(),
                parameters: HashMap::new(),
            },
        );
        models.insert(
            "heavy".to_string(),
            TierConfig {
                model: "gpt-4-turbo".to_string(),
                parameters: HashMap::new(),
            },
        );

        let config = CascadeConfig {
            enabled: true,
            routing_policy: RoutingPolicy::Conservative,
            default_tier: Some("medium".to_string()),
            models,
            confidence_threshold: 0.7,
        };

        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_get_model_for_tier() {
        let mut models = HashMap::new();
        models.insert(
            "medium".to_string(),
            TierConfig {
                model: "gpt-4".to_string(),
                parameters: HashMap::new(),
            },
        );

        let config = CascadeConfig {
            enabled: true,
            routing_policy: RoutingPolicy::Conservative,
            default_tier: None,
            models,
            confidence_threshold: 0.7,
        };

        let result = config.get_model_for_tier(ExecutionTier::Medium);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "gpt-4");
    }
}
