use super::*;

#[test]
fn default_config_has_no_theme() {
    let config = UserConfig::default();
    assert_eq!(config.theme.default, None);
}

#[test]
fn load_missing_file_returns_default() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    let config = load_from(&path).unwrap();
    assert_eq!(config, UserConfig::default());
}

#[test]
fn save_and_load_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        theme: ThemeConfig {
            default: Some("dracula".to_string()),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded, config);
    assert_eq!(loaded.theme.default.as_deref(), Some("dracula"));
}

#[test]
fn save_creates_parent_directories() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("nested").join("deep").join("config.toml");

    let config = UserConfig {
        theme: ThemeConfig {
            default: Some("tokyo-night".to_string()),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    assert!(path.is_file());
}

#[test]
fn load_invalid_toml_returns_error() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "this is not [valid toml").unwrap();

    let result = load_from(&path);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("parse"));
}

#[test]
fn load_empty_file_returns_default() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config, UserConfig::default());
}

#[test]
fn load_partial_config_fills_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.theme.default, None);
}

#[test]
fn save_produces_valid_toml() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        theme: ThemeConfig {
            default: Some("catppuccin-latte".to_string()),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("catppuccin-latte"));
    assert!(content.contains("[theme]"));
}

#[test]
fn default_config_has_no_budget() {
    let config = UserConfig::default();
    assert_eq!(config.budget.limit, None);
    assert_eq!(config.budget.warn_threshold, None);
}

#[test]
fn budget_defaults_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        budget: BudgetDefaults {
            limit: Some(10.0),
            warn_threshold: Some(7.5),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.budget.limit, Some(10.0));
    assert_eq!(loaded.budget.warn_threshold, Some(7.5));
}

#[test]
fn budget_defaults_partial_config() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[budget]\nlimit = 5.0\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.budget.limit, Some(5.0));
    assert_eq!(config.budget.warn_threshold, None);
}

#[test]
fn config_without_budget_section_uses_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\ndefault = \"dracula\"\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.budget, BudgetDefaults::default());
    assert_eq!(config.theme.default.as_deref(), Some("dracula"));
}

#[test]
fn budget_defaults_serializes_to_toml() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        budget: BudgetDefaults {
            limit: Some(20.0),
            warn_threshold: Some(15.0),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[budget]"));
    assert!(content.contains("20.0"));
    assert!(content.contains("15.0"));
}

#[test]
fn layout_config_default_has_no_grid() {
    let config = UserConfig::default();
    assert_eq!(config.layout.grid, None);
}

#[test]
fn layout_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        theme: ThemeConfig::default(),
        budget: BudgetDefaults::default(),
        layout: LayoutConfig {
            grid: Some("sidebar | stream | [stats, commits]".to_string()),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(
        loaded.layout.grid.as_deref(),
        Some("sidebar | stream | [stats, commits]")
    );
}

#[test]
fn config_without_layout_section_uses_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\ndefault = \"dracula\"\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.layout, LayoutConfig::default());
}

#[test]
fn layout_config_serializes_to_toml() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        theme: ThemeConfig::default(),
        budget: BudgetDefaults::default(),
        layout: LayoutConfig {
            grid: Some("stream | detail".to_string()),
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("[layout]"));
    assert!(content.contains("stream | detail"));
}

#[test]
fn error_display_messages() {
    let io_err = UserConfigError::Io(
        PathBuf::from("/tmp/test"),
        std::io::Error::new(std::io::ErrorKind::NotFound, "not found"),
    );
    assert!(io_err.to_string().contains("config I/O error"));

    let parse_err = UserConfigError::Parse(PathBuf::from("/tmp/test"), "bad syntax".to_string());
    assert!(parse_err.to_string().contains("parse config"));

    let ser_err = UserConfigError::Serialize("encoding failed".to_string());
    assert!(ser_err.to_string().contains("serialize config"));

    let home_err = UserConfigError::NoHome("no home".to_string());
    assert!(home_err.to_string().contains("no home"));
}

// ── Appearance config tests ──────────────────────────────────

#[test]
fn appearance_config_default() {
    let config = UserConfig::default();
    assert_eq!(config.appearance.easing, None);
    assert_eq!(config.appearance.border_style, None);
    assert_eq!(config.appearance.color_depth, None);
    assert_eq!(config.appearance.animation_speed, None);
}

#[test]
fn appearance_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        appearance: AppearanceConfig {
            easing: Some("spring".to_string()),
            border_style: Some("sharp".to_string()),
            color_depth: Some("truecolor".to_string()),
            animation_speed: Some("2x".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.appearance.easing.as_deref(), Some("spring"));
    assert_eq!(loaded.appearance.border_style.as_deref(), Some("sharp"));
    assert_eq!(loaded.appearance.color_depth.as_deref(), Some("truecolor"));
    assert_eq!(loaded.appearance.animation_speed.as_deref(), Some("2x"));
}

#[test]
fn appearance_config_defaults_helpers() {
    let config = AppearanceConfig::default();
    assert_eq!(config.border_style_or_default(), "rounded");
    assert_eq!(config.color_depth_or_default(), "auto");
    assert_eq!(config.animation_speed_or_default(), "1x");

    let config = AppearanceConfig {
        border_style: Some("double".to_string()),
        color_depth: Some("256".to_string()),
        animation_speed: Some("0.5x".to_string()),
        ..Default::default()
    };
    assert_eq!(config.border_style_or_default(), "double");
    assert_eq!(config.color_depth_or_default(), "256");
    assert_eq!(config.animation_speed_or_default(), "0.5x");
}

// ── Controls config tests ────────────────────────────────────

#[test]
fn controls_config_default() {
    let config = UserConfig::default();
    assert_eq!(config.controls.keybinding_preset, None);
    assert_eq!(config.controls.sound, None);
}

#[test]
fn controls_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        controls: ControlsConfig {
            keybinding_preset: Some("vim".to_string()),
            sound: Some(true),
            binding_overrides: None,
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.controls.keybinding_preset.as_deref(), Some("vim"));
    assert_eq!(loaded.controls.sound, Some(true));
}

#[test]
fn config_without_controls_section_uses_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\ndefault = \"dracula\"\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.controls, ControlsConfig::default());
}

// ── Accessibility config tests ───────────────────────────────

#[test]
fn accessibility_config_default() {
    let config = UserConfig::default();
    assert_eq!(config.accessibility.reduce_motion, None);
    assert_eq!(config.accessibility.high_contrast, None);
    assert_eq!(config.accessibility.cvd_mode, None);
    assert_eq!(config.accessibility.instant_scroll, None);
}

#[test]
fn accessibility_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        accessibility: AccessibilityDefaults {
            reduce_motion: Some(true),
            high_contrast: Some(false),
            cvd_mode: Some("deuteranopia".to_string()),
            instant_scroll: Some(true),
            screen_reader: None,
            focus_timeout: None,
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.accessibility.reduce_motion, Some(true));
    assert_eq!(loaded.accessibility.high_contrast, Some(false));
    assert_eq!(
        loaded.accessibility.cvd_mode.as_deref(),
        Some("deuteranopia")
    );
    assert_eq!(loaded.accessibility.instant_scroll, Some(true));
}

#[test]
fn config_without_accessibility_section_uses_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\ndefault = \"dracula\"\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.accessibility, AccessibilityDefaults::default());
}

#[test]
fn full_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        theme: ThemeConfig {
            default: Some("tokyo-night".to_string()),
        },
        budget: BudgetDefaults {
            limit: Some(10.0),
            warn_threshold: Some(7.5),
        },
        layout: LayoutConfig {
            grid: Some("sidebar | stream".to_string()),
        },
        appearance: AppearanceConfig {
            easing: Some("smooth".to_string()),
            border_style: Some("double".to_string()),
            color_depth: Some("256".to_string()),
            animation_speed: Some("1.5x".to_string()),
            ..Default::default()
        },
        controls: ControlsConfig {
            keybinding_preset: Some("emacs".to_string()),
            sound: Some(false),
            binding_overrides: None,
        },
        accessibility: AccessibilityDefaults {
            reduce_motion: Some(true),
            high_contrast: Some(true),
            cvd_mode: Some("protanopia".to_string()),
            instant_scroll: Some(false),
            screen_reader: Some(true),
            focus_timeout: Some(10),
        },
        models: ModelDefaults {
            claude: Some("claude-sonnet-4".to_string()),
            copilot: Some("gpt-4o".to_string()),
        },
        session: SessionDefaults {
            default_provider: Some("claude".to_string()),
            ..Default::default()
        },
        general: GeneralConfig {
            project_name: Some("my-project".to_string()),
            prompt_template: Some("Be concise".to_string()),
            max_iterations: Some(20),
            auto_pause_on_error: Some(false),
            visual_intensity: Some("full".to_string()),
            transition_delay_ms: None,
            ..Default::default()
        },
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded, config);
}

#[test]
fn for_provider_returns_model_when_set() {
    let models = ModelDefaults {
        claude: Some("claude-opus-4".to_string()),
        copilot: Some("gpt-4o".to_string()),
    };
    assert_eq!(models.for_provider("claude"), Some("claude-opus-4"));
    assert_eq!(models.for_provider("copilot"), Some("gpt-4o"));
}

#[test]
fn for_provider_returns_none_for_default_sentinel() {
    let models = ModelDefaults {
        claude: Some("default".to_string()),
        copilot: Some("Default".to_string()),
    };
    assert_eq!(models.for_provider("claude"), None);
    assert_eq!(models.for_provider("copilot"), None);
}

// ── Session config tests ────────────────────────────────────

#[test]
fn session_config_default() {
    let config = UserConfig::default();
    assert_eq!(config.session.default_provider, None);
    assert_eq!(config.session.provider(), None);
}

#[test]
fn session_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        session: SessionDefaults {
            default_provider: Some("claude".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.session.default_provider.as_deref(), Some("claude"));
    assert_eq!(loaded.session.provider(), Some("claude"));
}

#[test]
fn session_provider_returns_none_for_auto() {
    let session = SessionDefaults {
        default_provider: Some("auto".to_string()),
        ..Default::default()
    };
    assert_eq!(session.provider(), None);

    let session_upper = SessionDefaults {
        default_provider: Some("Auto".to_string()),
        ..Default::default()
    };
    assert_eq!(session_upper.provider(), None);
}

#[test]
fn session_provider_returns_none_when_empty() {
    let session = SessionDefaults {
        default_provider: Some(String::new()),
        ..Default::default()
    };
    assert_eq!(session.provider(), None);
}

#[test]
fn session_provider_returns_copilot() {
    let session = SessionDefaults {
        default_provider: Some("copilot".to_string()),
        ..Default::default()
    };
    assert_eq!(session.provider(), Some("copilot"));
}

#[test]
fn config_without_session_section_uses_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\ndefault = \"dracula\"\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.session, SessionDefaults::default());
}

#[test]
fn for_provider_returns_none_when_unset() {
    let models = ModelDefaults::default();
    assert_eq!(models.for_provider("claude"), None);
    assert_eq!(models.for_provider("copilot"), None);
    assert_eq!(models.for_provider("unknown"), None);
}

// ── General config tests ─────────────────────────────────────

#[test]
fn general_config_default() {
    let config = UserConfig::default();
    assert_eq!(config.general.project_name, None);
    assert_eq!(config.general.prompt_template, None);
    assert_eq!(config.general.max_iterations, None);
    assert_eq!(config.general.auto_pause_on_error, None);
    assert_eq!(config.general.visual_intensity, None);
}

#[test]
fn general_config_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        general: GeneralConfig {
            project_name: Some("test-project".to_string()),
            prompt_template: Some("Be helpful".to_string()),
            max_iterations: Some(25),
            auto_pause_on_error: Some(false),
            visual_intensity: Some("minimal".to_string()),
            transition_delay_ms: Some(1500),
            ..Default::default()
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.general, config.general);
}

#[test]
fn general_config_defaults_helpers() {
    let general = GeneralConfig::default();
    assert_eq!(general.max_iterations_or_default(), 10);
    assert!(general.auto_pause_or_default());
    assert_eq!(general.visual_intensity_or_default(), "balanced");
    assert_eq!(general.transition_delay_or_default(), 2000);

    let custom = GeneralConfig {
        max_iterations: Some(5),
        auto_pause_on_error: Some(false),
        visual_intensity: Some("full".to_string()),
        transition_delay_ms: Some(500),
        ..Default::default()
    };
    assert_eq!(custom.max_iterations_or_default(), 5);
    assert!(!custom.auto_pause_or_default());
    assert_eq!(custom.visual_intensity_or_default(), "full");
    assert_eq!(custom.transition_delay_or_default(), 500);
}

#[test]
fn config_without_general_section_uses_defaults() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");
    std::fs::write(&path, "[theme]\ndefault = \"dracula\"\n").unwrap();

    let config = load_from(&path).unwrap();
    assert_eq!(config.general, GeneralConfig::default());
}

// ── Agent isolation config tests ─────────────────────────────

#[test]
fn session_isolation_mode_default() {
    let session = SessionDefaults::default();
    assert_eq!(session.isolation_mode, None);
    assert_eq!(session.isolation_mode_or_default(), "sandbox");
}

#[test]
fn session_isolation_mode_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        session: SessionDefaults {
            isolation_mode: Some("augmented".to_string()),
            permission_mode: Some("plan".to_string()),
            system_prompt_append: Some("Be extra careful".to_string()),
            extra_add_dirs: Some(vec!["/tmp/extra".to_string()]),
            ..Default::default()
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    assert_eq!(loaded.session.isolation_mode.as_deref(), Some("augmented"));
    assert_eq!(loaded.session.isolation_mode_or_default(), "augmented");
    assert_eq!(loaded.session.permission_mode(), Some("plan"));
    assert_eq!(
        loaded.session.system_prompt_append.as_deref(),
        Some("Be extra careful")
    );
    assert_eq!(
        loaded.session.extra_add_dirs,
        Some(vec!["/tmp/extra".to_string()])
    );
}

#[test]
fn session_permission_mode_returns_none_when_empty() {
    let session = SessionDefaults {
        permission_mode: Some(String::new()),
        ..Default::default()
    };
    assert_eq!(session.permission_mode(), None);
}

#[test]
fn session_mcp_servers_round_trip() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig {
        session: SessionDefaults {
            extra_mcp_servers: Some(vec![McpServerEntry {
                name: "magistr".to_string(),
                command: "magistr-mcp".to_string(),
                args: vec!["serve".to_string()],
                env: [("KEY".to_string(), "val".to_string())]
                    .into_iter()
                    .collect(),
            }]),
            ..Default::default()
        },
        ..Default::default()
    };

    save_to(&config, &path).unwrap();
    let loaded = load_from(&path).unwrap();
    let servers = loaded.session.extra_mcp_servers.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "magistr");
    assert_eq!(servers[0].command, "magistr-mcp");
    assert_eq!(servers[0].args, vec!["serve"]);
    assert_eq!(servers[0].env.get("KEY").map(String::as_str), Some("val"));
}

#[test]
fn session_no_mcp_servers_omitted_in_toml() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("config.toml");

    let config = UserConfig::default();
    save_to(&config, &path).unwrap();
    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.contains("extra_mcp_servers"));
}
