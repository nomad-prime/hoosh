// Comprehensive unit tests for Config module
// Tests focus on configuration loading, merging, validation, and error handling

use super::*;

#[test]
fn default_config_has_required_fields() {
    let config = AppConfig::default();

    assert_eq!(config.default_backend, "mock");
    assert_eq!(config.default_agent, Some("hoosh_coder".to_string()));
    assert!(config.backends.is_empty());
    assert!(!config.agents.is_empty());
}

#[test]
fn default_config_loads_agents_from_prompts() {
    let config = AppConfig::default();

    // Should have loaded agents from src/prompts directory
    assert!(!config.agents.is_empty());

    // Check that at least one expected agent exists
    assert!(config.agents.contains_key("hoosh_coder"));
}

#[test]
fn backend_config_fields_are_optional() {
    let backend = BackendConfig {
        api_key: None,
        model: None,
        base_url: None,
        chat_api: None,
        temperature: None,
    };

    assert!(backend.api_key.is_none());
    assert!(backend.model.is_none());
    assert!(backend.base_url.is_none());
    assert!(backend.chat_api.is_none());
    assert!(backend.temperature.is_none());
}

#[test]
fn backend_config_can_be_fully_populated() {
    let backend = BackendConfig {
        api_key: Some("test-key".to_string()),
        model: Some("gpt-4".to_string()),
        base_url: Some("https://api.example.com".to_string()),
        chat_api: Some("chat".to_string()),
        temperature: Some(0.7),
    };

    assert_eq!(backend.api_key, Some("test-key".to_string()));
    assert_eq!(backend.model, Some("gpt-4".to_string()));
    assert_eq!(
        backend.base_url,
        Some("https://api.example.com".to_string())
    );
    assert_eq!(backend.chat_api, Some("chat".to_string()));
    assert_eq!(backend.temperature, Some(0.7));
}

#[test]
fn agent_config_has_file_and_optional_fields() {
    let agent = AgentConfig {
        file: "test.txt".to_string(),
        description: Some("Test agent".to_string()),
        tags: vec!["coding".to_string(), "debug".to_string()],
        core_instructions_file: None,
    };

    assert_eq!(agent.file, "test.txt");
    assert_eq!(agent.description, Some("Test agent".to_string()));
    assert_eq!(agent.tags.len(), 2);
}

#[test]
fn agent_config_tags_default_to_empty() {
    let agent = AgentConfig {
        file: "test.txt".to_string(),
        description: None,
        tags: vec![],
        core_instructions_file: None,
    };

    assert!(agent.tags.is_empty());
}

#[test]
fn project_config_defaults_are_empty() {
    let project_config = ProjectConfig::default();

    assert!(project_config.default_backend.is_none());
    assert!(project_config.backends.is_empty());
    assert!(project_config.verbosity.is_none());
    assert!(project_config.default_agent.is_none());
    assert!(project_config.agents.is_empty());
    assert!(project_config.context_manager.is_none());
}

#[test]
fn get_backend_config_returns_none_when_not_found() {
    let config = AppConfig::default();

    assert!(config.get_backend_config("nonexistent").is_none());
}

#[test]
fn get_backend_config_returns_config_when_exists() {
    let mut config = AppConfig::default();
    let backend = BackendConfig {
        api_key: Some("key".to_string()),
        model: None,
        base_url: None,
        chat_api: None,
        temperature: None,
    };

    config.set_backend_config("test".to_string(), backend);

    let retrieved = config.get_backend_config("test");
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().api_key, Some("key".to_string()));
}

#[test]
fn set_backend_config_adds_new_backend() {
    let mut config = AppConfig::default();
    let backend = BackendConfig {
        api_key: Some("new-key".to_string()),
        model: Some("model-1".to_string()),
        base_url: None,
        chat_api: None,
        temperature: None,
    };

    config.set_backend_config("new_backend".to_string(), backend);

    assert!(config.backends.contains_key("new_backend"));
    assert_eq!(
        config.get_backend_config("new_backend").unwrap().api_key,
        Some("new-key".to_string())
    );
}

#[test]
fn update_backend_setting_creates_backend_if_not_exists() {
    let mut config = AppConfig::default();

    config
        .update_backend_setting("new", "api_key", "test-key".to_string())
        .unwrap();

    assert!(config.backends.contains_key("new"));
    assert_eq!(
        config.get_backend_config("new").unwrap().api_key,
        Some("test-key".to_string())
    );
}

#[test]
fn update_backend_setting_updates_api_key() {
    let mut config = AppConfig::default();

    config
        .update_backend_setting("test", "api_key", "my-key".to_string())
        .unwrap();

    assert_eq!(
        config.get_backend_config("test").unwrap().api_key,
        Some("my-key".to_string())
    );
}

#[test]
fn update_backend_setting_updates_model() {
    let mut config = AppConfig::default();

    config
        .update_backend_setting("test", "model", "gpt-4".to_string())
        .unwrap();

    assert_eq!(
        config.get_backend_config("test").unwrap().model,
        Some("gpt-4".to_string())
    );
}

#[test]
fn update_backend_setting_updates_base_url() {
    let mut config = AppConfig::default();

    config
        .update_backend_setting("test", "base_url", "https://api.test.com".to_string())
        .unwrap();

    assert_eq!(
        config.get_backend_config("test").unwrap().base_url,
        Some("https://api.test.com".to_string())
    );
}

#[test]
fn update_backend_setting_updates_chat_api() {
    let mut config = AppConfig::default();

    config
        .update_backend_setting("test", "chat_api", "v1/chat".to_string())
        .unwrap();

    assert_eq!(
        config.get_backend_config("test").unwrap().chat_api,
        Some("v1/chat".to_string())
    );
}

#[test]
fn update_backend_setting_updates_temperature() {
    let mut config = AppConfig::default();

    config
        .update_backend_setting("test", "temperature", "0.8".to_string())
        .unwrap();

    assert_eq!(
        config.get_backend_config("test").unwrap().temperature,
        Some(0.8)
    );
}

#[test]
fn update_backend_setting_rejects_invalid_temperature() {
    let mut config = AppConfig::default();

    let result = config.update_backend_setting("test", "temperature", "invalid".to_string());

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ConfigError::InvalidValue { .. }
    ));
}

#[test]
fn update_backend_setting_rejects_unknown_key() {
    let mut config = AppConfig::default();

    let result = config.update_backend_setting("test", "unknown_key", "value".to_string());

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        ConfigError::UnknownConfigKey { .. }
    ));
}

#[test]
fn get_verbosity_returns_normal_by_default() {
    let config = AppConfig::default();

    assert_eq!(config.get_verbosity(), VerbosityLevel::Normal);
}

#[test]
fn get_verbosity_parses_quiet() {
    let config = AppConfig {
        verbosity: Some("quiet".to_string()),
        ..Default::default()
    };

    assert_eq!(config.get_verbosity(), VerbosityLevel::Quiet);
}

#[test]
fn get_verbosity_parses_normal() {
    let config = AppConfig {
        verbosity: Some("normal".to_string()),
        ..Default::default()
    };

    assert_eq!(config.get_verbosity(), VerbosityLevel::Normal);
}

#[test]
fn get_verbosity_parses_verbose() {
    let config = AppConfig {
        verbosity: Some("verbose".to_string()),
        ..Default::default()
    };

    assert_eq!(config.get_verbosity(), VerbosityLevel::Verbose);
}

#[test]
fn get_verbosity_parses_debug() {
    let config = AppConfig {
        verbosity: Some("debug".to_string()),
        ..Default::default()
    };

    assert_eq!(config.get_verbosity(), VerbosityLevel::Debug);
}

#[test]
fn get_verbosity_fallback_on_invalid() {
    let config = AppConfig {
        verbosity: Some("invalid".to_string()),
        ..Default::default()
    };

    assert_eq!(config.get_verbosity(), VerbosityLevel::Normal);
}

#[test]
fn set_verbosity_updates_config() {
    let mut config = AppConfig::default();

    config.set_verbosity(VerbosityLevel::Debug);

    assert_eq!(config.verbosity, Some("debug".to_string()));
}

#[test]
fn set_default_agent_updates_config() {
    let mut config = AppConfig::default();

    config.set_default_agent("new_agent".to_string());

    assert_eq!(config.default_agent, Some("new_agent".to_string()));
}

#[test]
fn get_context_manager_config_returns_default() {
    let config = AppConfig::default();

    let ctx_config = config.get_context_manager_config();

    // Should return default ContextManagerConfig
    assert_eq!(ctx_config, ContextManagerConfig::default());
}

#[test]
fn merge_overwrites_backends() {
    let mut config = AppConfig::default();
    config.backends.insert(
        "test".to_string(),
        BackendConfig {
            api_key: Some("old".to_string()),
            model: None,
            base_url: None,
            chat_api: None,
            temperature: None,
        },
    );

    let mut project_config = ProjectConfig::default();
    project_config.backends.insert(
        "test".to_string(),
        BackendConfig {
            api_key: Some("new".to_string()),
            model: None,
            base_url: None,
            chat_api: None,
            temperature: None,
        },
    );

    config.merge(project_config);

    assert_eq!(
        config.get_backend_config("test").unwrap().api_key,
        Some("new".to_string())
    );
}

#[test]
fn merge_overwrites_agents() {
    let mut config = AppConfig::default();
    config.agents.insert(
        "agent1".to_string(),
        AgentConfig {
            file: "old.txt".to_string(),
            description: None,
            tags: vec![],
            core_instructions_file: None,
        },
    );

    let mut project_config = ProjectConfig::default();
    project_config.agents.insert(
        "agent1".to_string(),
        AgentConfig {
            file: "new.txt".to_string(),
            description: Some("Updated".to_string()),
            tags: vec![],
            core_instructions_file: None,
        },
    );

    config.merge(project_config);

    assert_eq!(config.agents.get("agent1").unwrap().file, "new.txt");
    assert_eq!(
        config.agents.get("agent1").unwrap().description,
        Some("Updated".to_string())
    );
}

#[test]
fn merge_updates_default_backend() {
    let mut config = AppConfig {
        default_backend: "old_backend".to_string(),
        ..Default::default()
    };

    let project_config = ProjectConfig {
        default_backend: Some("new_backend".to_string()),
        ..Default::default()
    };

    config.merge(project_config);

    assert_eq!(config.default_backend, "new_backend");
}

#[test]
fn merge_ignores_empty_default_backend() {
    let mut config = AppConfig {
        default_backend: "old_backend".to_string(),
        ..Default::default()
    };

    let project_config = ProjectConfig {
        default_backend: Some("".to_string()),
        ..Default::default()
    };

    config.merge(project_config);

    assert_eq!(config.default_backend, "old_backend");
}

#[test]
fn merge_updates_verbosity() {
    let mut config = AppConfig {
        verbosity: Some("quiet".to_string()),
        ..Default::default()
    };

    let project_config = ProjectConfig {
        verbosity: Some("debug".to_string()),
        ..Default::default()
    };

    config.merge(project_config);

    assert_eq!(config.verbosity, Some("debug".to_string()));
}

#[test]
fn merge_updates_default_agent() {
    let mut config = AppConfig {
        default_agent: Some("old_agent".to_string()),
        ..Default::default()
    };

    let project_config = ProjectConfig {
        default_agent: Some("new_agent".to_string()),
        ..Default::default()
    };

    config.merge(project_config);

    assert_eq!(config.default_agent, Some("new_agent".to_string()));
}

#[test]
fn merge_updates_context_manager() {
    let mut config = AppConfig::default();
    let old_ctx = ContextManagerConfig::default();
    config.context_manager = Some(old_ctx);

    let mut project_config = ProjectConfig::default();
    let new_ctx = ContextManagerConfig::default();
    project_config.context_manager = Some(new_ctx.clone());

    config.merge(project_config);

    assert_eq!(config.context_manager, Some(new_ctx));
}

#[test]
fn config_path_uses_home_directory() {
    let path = AppConfig::config_path();

    assert!(path.is_ok());
    let path = path.unwrap();
    assert!(path.to_str().unwrap().contains(".config"));
    assert!(path.to_str().unwrap().contains("hoosh"));
    assert!(path.to_str().unwrap().ends_with("config.toml"));
}

#[test]
fn project_config_path_uses_current_directory() {
    let path = AppConfig::project_config_path();

    assert!(path.is_ok());
    let path = path.unwrap();
    assert!(path.to_str().unwrap().contains(".hoosh"));
    assert!(path.to_str().unwrap().ends_with("config.toml"));
}

#[test]
fn serialize_backend_config_to_toml() {
    let backend = BackendConfig {
        api_key: Some("test-key".to_string()),
        model: Some("gpt-4".to_string()),
        base_url: None,
        chat_api: None,
        temperature: Some(0.7),
    };

    let toml = toml::to_string(&backend).unwrap();

    assert!(toml.contains("api_key"));
    assert!(toml.contains("test-key"));
    assert!(toml.contains("model"));
    assert!(toml.contains("gpt-4"));
    assert!(toml.contains("temperature"));
}

#[test]
fn deserialize_backend_config_from_toml() {
    let toml = r#"
        api_key = "test-key"
        model = "gpt-4"
        temperature = 0.7
    "#;

    let backend: BackendConfig = toml::from_str(toml).unwrap();

    assert_eq!(backend.api_key, Some("test-key".to_string()));
    assert_eq!(backend.model, Some("gpt-4".to_string()));
    assert_eq!(backend.temperature, Some(0.7));
}

#[test]
fn serialize_agent_config_to_toml() {
    let agent = AgentConfig {
        file: "coder.txt".to_string(),
        description: Some("Coding assistant".to_string()),
        tags: vec!["coding".to_string(), "rust".to_string()],
        core_instructions_file: None,
    };

    let toml = toml::to_string(&agent).unwrap();

    assert!(toml.contains("file"));
    assert!(toml.contains("coder.txt"));
    assert!(toml.contains("description"));
    assert!(toml.contains("Coding assistant"));
    assert!(toml.contains("tags"));
}

#[test]
fn deserialize_agent_config_from_toml() {
    let toml = r#"
        file = "coder.txt"
        description = "Coding assistant"
        tags = ["coding", "rust"]
    "#;

    let agent: AgentConfig = toml::from_str(toml).unwrap();

    assert_eq!(agent.file, "coder.txt");
    assert_eq!(agent.description, Some("Coding assistant".to_string()));
    assert_eq!(agent.tags, vec!["coding", "rust"]);
}

#[test]
fn deserialize_agent_config_with_defaults() {
    let toml = r#"
        file = "simple.txt"
    "#;

    let agent: AgentConfig = toml::from_str(toml).unwrap();

    assert_eq!(agent.file, "simple.txt");
    assert_eq!(agent.description, None);
    assert!(agent.tags.is_empty());
}

#[test]
fn serialize_app_config_to_toml() {
    let config = AppConfig {
        default_backend: "openai".to_string(),
        verbosity: Some("verbose".to_string()),
        ..Default::default()
    };

    let toml = toml::to_string(&config).unwrap();

    assert!(toml.contains("default_backend"));
    assert!(toml.contains("openai"));
    assert!(toml.contains("verbosity"));
    assert!(toml.contains("verbose"));
}

#[test]
fn deserialize_app_config_from_toml() {
    let toml = r#"
        default_backend = "openai"
        verbosity = "debug"
        default_agent = "coder"

        [backends.openai]
        api_key = "sk-test"
        model = "gpt-4"

        [agents.coder]
        file = "coder.txt"
        description = "Coding assistant"
    "#;

    let config: AppConfig = toml::from_str(toml).unwrap();

    assert_eq!(config.default_backend, "openai");
    assert_eq!(config.verbosity, Some("debug".to_string()));
    assert_eq!(config.default_agent, Some("coder".to_string()));
    assert!(config.backends.contains_key("openai"));
    assert!(config.agents.contains_key("coder"));
}

#[test]
fn config_error_not_found_has_path() {
    let path = PathBuf::from("/test/path");
    let error = ConfigError::NotFound { path: path.clone() };

    let error_msg = format!("{}", error);
    assert!(error_msg.contains("not found"));
    assert!(error_msg.contains("/test/path"));
}

#[test]
fn config_error_invalid_value_shows_details() {
    let error = ConfigError::InvalidValue {
        field: "temperature".to_string(),
        value: "invalid".to_string(),
    };

    let error_msg = format!("{}", error);
    assert!(error_msg.contains("temperature"));
    assert!(error_msg.contains("invalid"));
}

#[test]
fn config_error_unknown_key_shows_key() {
    let error = ConfigError::UnknownConfigKey {
        key: "unknown_field".to_string(),
    };

    let error_msg = format!("{}", error);
    assert!(error_msg.contains("unknown_field"));
}

#[test]
fn clone_backend_config() {
    let backend = BackendConfig {
        api_key: Some("key".to_string()),
        model: Some("model".to_string()),
        base_url: None,
        chat_api: None,
        temperature: Some(0.5),
    };

    let cloned = backend.clone();

    assert_eq!(backend.api_key, cloned.api_key);
    assert_eq!(backend.model, cloned.model);
    assert_eq!(backend.temperature, cloned.temperature);
}

#[test]
fn clone_agent_config() {
    let agent = AgentConfig {
        file: "test.txt".to_string(),
        description: Some("Test".to_string()),
        tags: vec!["tag1".to_string()],
        core_instructions_file: None,
    };

    let cloned = agent.clone();

    assert_eq!(agent.file, cloned.file);
    assert_eq!(agent.description, cloned.description);
    assert_eq!(agent.tags, cloned.tags);
    assert_eq!(agent.core_instructions_file, cloned.core_instructions_file);
}

#[test]
fn clone_app_config() {
    let config = AppConfig::default();
    let cloned = config.clone();

    assert_eq!(config.default_backend, cloned.default_backend);
    assert_eq!(config.verbosity, cloned.verbosity);
    assert_eq!(config.default_agent, cloned.default_agent);
}

#[test]
fn debug_format_backend_config() {
    let backend = BackendConfig {
        api_key: Some("key".to_string()),
        model: None,
        base_url: None,
        chat_api: None,
        temperature: None,
    };

    let debug_str = format!("{:?}", backend);

    assert!(debug_str.contains("BackendConfig"));
    assert!(debug_str.contains("api_key"));
}

#[test]
fn debug_format_agent_config() {
    let agent = AgentConfig {
        file: "test.txt".to_string(),
        description: None,
        tags: vec![],
        core_instructions_file: None,
    };

    let debug_str = format!("{:?}", agent);

    assert!(debug_str.contains("AgentConfig"));
    assert!(debug_str.contains("file"));
}
