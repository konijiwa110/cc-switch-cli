use super::*;
use serial_test::serial;
use std::ffi::OsString;
use std::path::Path;
use tempfile::TempDir;

struct EnvGuard {
    old_home: Option<OsString>,
    old_userprofile: Option<OsString>,
}

impl EnvGuard {
    fn set_home(home: &Path) -> Self {
        let old_home = std::env::var_os("HOME");
        let old_userprofile = std::env::var_os("USERPROFILE");
        std::env::set_var("HOME", home);
        std::env::set_var("USERPROFILE", home);
        Self {
            old_home,
            old_userprofile,
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        match &self.old_userprofile {
            Some(value) => std::env::set_var("USERPROFILE", value),
            None => std::env::remove_var("USERPROFILE"),
        }
    }
}

#[test]
fn validate_provider_settings_allows_missing_auth_for_codex() {
    let mut provider = Provider::with_id(
        "codex".into(),
        "Codex".into(),
        json!({ "config": "base_url = \"https://example.com\"" }),
        None,
    );
    provider.meta = Some(crate::provider::ProviderMeta {
        codex_official: Some(true),
        ..Default::default()
    });
    ProviderService::validate_provider_settings(&AppType::Codex, &provider)
        .expect("Codex auth is optional for official provider");
}

#[test]
fn validate_provider_settings_allows_missing_auth_for_codex_official_by_category() {
    let mut provider = Provider::with_id(
        "codex".into(),
        "Anything".into(),
        json!({ "config": "base_url = \"https://api.openai.com/v1\"\n" }),
        None,
    );
    provider.category = Some("official".to_string());
    ProviderService::validate_provider_settings(&AppType::Codex, &provider)
        .expect("Codex auth is optional for official providers (category=official)");
}

#[test]
#[serial]
fn switch_codex_succeeds_without_auth_json() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::codex_config::get_codex_config_dir())
        .expect("create ~/.codex (initialized)");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "p2".to_string();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "Keyring".to_string(),
                json!({
                    "config": "model_provider = \"keyring\"\nmodel = \"gpt-5.2-codex\"\n\n[model_providers.keyring]\nrequires_openai_auth = true\n",
                }),
                None,
            ),
        );
        manager.providers.insert(
            "p2".to_string(),
            Provider::with_id(
                "p2".to_string(),
                "Other".to_string(),
                json!({
                    "config": "model_provider = \"other\"\nmodel = \"gpt-5.2-codex\"\n\n[model_providers.other]\nrequires_openai_auth = true\n",
                }),
                None,
            ),
        );
    }

    let state = state_from_config(config);

    ProviderService::switch(&state, AppType::Codex, "p1")
        .expect("switch should succeed without auth.json when using credential store");

    assert!(
        !get_codex_auth_path().exists(),
        "auth.json should remain absent when provider has no auth config"
    );

    let live_config_text =
        std::fs::read_to_string(get_codex_config_path()).expect("read live config.toml");

    let guard = state.config.read().expect("read config after switch");
    let manager = guard
        .get_manager(&AppType::Codex)
        .expect("codex manager after switch");
    assert_eq!(manager.current, "p1", "current provider should update");
    let provider = manager.providers.get("p1").expect("p1 exists");
    assert!(
        provider.settings_config.get("auth").is_none(),
        "snapshot should not inject auth when auth.json is absent"
    );
    // After the switch, the stored config should match the live config.toml
    let stored_config = provider
        .settings_config
        .get("config")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        !stored_config.is_empty() || !live_config_text.trim().is_empty(),
        "provider snapshot should have config text after switch"
    );
}

#[test]
#[serial]
fn codex_switch_removes_existing_auth_json_for_openai_official_provider() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::codex_config::get_codex_config_dir())
        .expect("create ~/.codex (initialized)");

    // Seed an existing auth.json (simulates `codex login` or prior configuration).
    let existing_auth = json!({ "OPENAI_API_KEY": "sk-existing" });
    let auth_path = crate::codex_config::get_codex_auth_path();
    crate::config::write_json_file(&auth_path, &existing_auth).expect("write auth.json");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "p1".to_string();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "Third Party".to_string(),
                json!({
                    "auth": { "OPENAI_API_KEY": "sk-third-party" },
                    "config": "model_provider = \"thirdparty\"\nmodel = \"gpt-5.2-codex\"\n\n[model_providers.thirdparty]\nbase_url = \"https://third-party.example/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n",
                }),
                None,
            ),
        );

        let mut official = Provider::with_id(
            "p2".to_string(),
            "OpenAI Official".to_string(),
            json!({
                "config": "model_provider = \"openai\"\nmodel = \"gpt-5.2-codex\"\n\n[model_providers.openai]\nbase_url = \"https://api.openai.com/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n",
            }),
            None,
        );
        official.meta = Some(crate::provider::ProviderMeta {
            codex_official: Some(true),
            ..Default::default()
        });
        manager.providers.insert("p2".to_string(), official);
    }

    let state = state_from_config(config);

    ProviderService::switch(&state, AppType::Codex, "p2")
        .expect("switch to official should succeed");

    assert!(
        !auth_path.exists(),
        "auth.json should be removed when switching to OpenAI official provider"
    );

    let backup_exists = std::fs::read_dir(crate::codex_config::get_codex_config_dir())
        .expect("read codex dir")
        .filter_map(Result::ok)
        .any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("auth.json.cc-switch.bak.")
        });
    assert!(
        backup_exists,
        "auth.json should be backed up when removed for OpenAI official provider"
    );
}

#[test]
#[serial]
fn codex_switch_preserves_base_url_and_wire_api_across_multiple_switches() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::codex_config::get_codex_config_dir())
        .expect("create ~/.codex (initialized)");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "p1".to_string();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "Provider One".to_string(),
                json!({
                    "auth": { "OPENAI_API_KEY": "sk-one" },
                    "config": "model_provider = \"providerone\"\nmodel = \"gpt-4o\"\n\n[model_providers.providerone]\nbase_url = \"https://api.one.example/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n",
                }),
                None,
            ),
        );
        manager.providers.insert(
            "p2".to_string(),
            Provider::with_id(
                "p2".to_string(),
                "Provider Two".to_string(),
                json!({
                    "auth": { "OPENAI_API_KEY": "sk-two" },
                    "config": "model_provider = \"providertwo\"\nmodel = \"gpt-4o\"\n\n[model_providers.providertwo]\nbase_url = \"https://api.two.example/v1\"\nwire_api = \"chat\"\nrequires_openai_auth = true\n",
                }),
                None,
            ),
        );
    }

    let state = state_from_config(config);

    // Seed initial live config for p1, then switch to p2, then back to p1.
    ProviderService::switch(&state, AppType::Codex, "p1").expect("seed p1 live");
    ProviderService::switch(&state, AppType::Codex, "p2").expect("switch to p2");
    ProviderService::switch(&state, AppType::Codex, "p1").expect("switch back to p1");

    let live_text =
        std::fs::read_to_string(get_codex_config_path()).expect("read live config.toml");
    assert!(
        live_text.contains("base_url = \"https://api.one.example/v1\""),
        "live config should retain provider base_url after multiple switches"
    );
    assert!(
        live_text.contains("wire_api = \"responses\""),
        "live config should retain provider wire_api after multiple switches"
    );

    let guard = state.config.read().expect("read config");
    let manager = guard.get_manager(&AppType::Codex).expect("codex manager");
    let provider = manager.providers.get("p1").expect("p1 exists");
    let cfg = provider
        .settings_config
        .get("config")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        cfg.contains("base_url = \"https://api.one.example/v1\""),
        "provider snapshot should retain base_url across switches"
    );
    assert!(
        cfg.contains("wire_api = \"responses\""),
        "provider snapshot should retain wire_api across switches"
    );
}

#[test]
#[serial]
fn add_first_provider_sets_current() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    let state = state_from_config(config);

    let provider = Provider::with_id(
        "p1".to_string(),
        "First".to_string(),
        json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token",
                "ANTHROPIC_BASE_URL": "https://claude.example"
            }
        }),
        None,
    );

    ProviderService::add(&state, AppType::Claude, provider).expect("add should succeed");

    let cfg = state.config.read().expect("read config");
    let manager = cfg.get_manager(&AppType::Claude).expect("claude manager");
    assert_eq!(
        manager.current, "p1",
        "first provider should become current to avoid empty current provider"
    );
}

#[test]
#[serial]
fn current_self_heals_when_current_provider_missing() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    {
        let manager = config
            .get_manager_mut(&AppType::Claude)
            .expect("claude manager");
        manager.current = "missing".to_string();

        let mut p1 = Provider::with_id(
            "p1".to_string(),
            "First".to_string(),
            json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "token1",
                    "ANTHROPIC_BASE_URL": "https://claude.one"
                }
            }),
            None,
        );
        p1.sort_index = Some(10);

        let mut p2 = Provider::with_id(
            "p2".to_string(),
            "Second".to_string(),
            json!({
                "env": {
                    "ANTHROPIC_AUTH_TOKEN": "token2",
                    "ANTHROPIC_BASE_URL": "https://claude.two"
                }
            }),
            None,
        );
        p2.sort_index = Some(0);

        manager.providers.insert("p1".to_string(), p1);
        manager.providers.insert("p2".to_string(), p2);
    }

    let state = state_from_config(config);

    let current_id =
        ProviderService::current(&state, AppType::Claude).expect("self-heal current provider");
    assert_eq!(
        current_id, "p2",
        "should pick provider with smaller sort_index"
    );

    let cfg = state.config.read().expect("read config");
    let manager = cfg.get_manager(&AppType::Claude).expect("claude manager");
    assert_eq!(
        manager.current, "p2",
        "current should be updated in config after self-heal"
    );
}

#[test]
#[serial]
fn common_config_snippet_is_merged_into_claude_settings_on_write() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::config::get_claude_config_dir())
        .expect("create ~/.claude (initialized)");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    config.common_config_snippets.claude = Some(
        r#"{"env":{"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":1},"includeCoAuthoredBy":false}"#
            .to_string(),
    );

    let state = state_from_config(config);

    let provider = Provider::with_id(
        "p1".to_string(),
        "First".to_string(),
        json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token",
                "ANTHROPIC_BASE_URL": "https://claude.example"
            }
        }),
        None,
    );

    ProviderService::add(&state, AppType::Claude, provider).expect("add should succeed");

    let settings_path = get_claude_settings_path();
    let live: Value = read_json_file(&settings_path).expect("read live settings");

    assert_eq!(
        live.get("includeCoAuthoredBy").and_then(Value::as_bool),
        Some(false),
        "common snippet should be merged into settings.json"
    );

    let env = live
        .get("env")
        .and_then(Value::as_object)
        .expect("settings.env should be object");

    assert_eq!(
        env.get("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC")
            .and_then(Value::as_i64),
        Some(1),
        "common env key should be present in settings.env"
    );
    assert_eq!(
        env.get("ANTHROPIC_AUTH_TOKEN").and_then(Value::as_str),
        Some("token"),
        "provider env key should remain in settings.env"
    );
}

#[test]
#[serial]
fn common_config_snippet_can_be_disabled_per_provider_for_claude() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::config::get_claude_config_dir())
        .expect("create ~/.claude (initialized)");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    config.common_config_snippets.claude = Some(
        r#"{"env":{"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":1},"includeCoAuthoredBy":false}"#
            .to_string(),
    );

    let state = state_from_config(config);

    let provider: Provider = serde_json::from_value(json!({
        "id": "p1",
        "name": "First",
        "settingsConfig": {
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token",
                "ANTHROPIC_BASE_URL": "https://claude.example"
            }
        },
        "meta": { "applyCommonConfig": false }
    }))
    .expect("parse provider");

    ProviderService::add(&state, AppType::Claude, provider).expect("add should succeed");

    let settings_path = get_claude_settings_path();
    let live: Value = read_json_file(&settings_path).expect("read live settings");

    assert!(
        live.get("includeCoAuthoredBy").is_none(),
        "common snippet should not be merged when applyCommonConfig=false"
    );
    assert!(
        !live
            .get("env")
            .and_then(Value::as_object)
            .map(|env| env.contains_key("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"))
            .unwrap_or(false),
        "common env keys should not be merged when applyCommonConfig=false"
    );
    assert_eq!(
        live.get("env")
            .and_then(Value::as_object)
            .and_then(|env| env.get("ANTHROPIC_AUTH_TOKEN"))
            .and_then(Value::as_str),
        Some("token"),
        "provider env should still be written"
    );
}

#[test]
#[serial]
fn common_config_snippet_is_not_persisted_into_provider_snapshot_on_switch() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Claude);
    config.common_config_snippets.claude = Some(
        r#"{"env":{"CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC":1},"includeCoAuthoredBy":false}"#
            .to_string(),
    );

    let state = state_from_config(config);

    let p1 = Provider::with_id(
        "p1".to_string(),
        "First".to_string(),
        json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token1",
                "ANTHROPIC_BASE_URL": "https://claude.one"
            }
        }),
        None,
    );
    let p2 = Provider::with_id(
        "p2".to_string(),
        "Second".to_string(),
        json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token2",
                "ANTHROPIC_BASE_URL": "https://claude.two"
            }
        }),
        None,
    );

    ProviderService::add(&state, AppType::Claude, p1).expect("add p1");
    ProviderService::add(&state, AppType::Claude, p2).expect("add p2");

    ProviderService::switch(&state, AppType::Claude, "p2").expect("switch to p2");

    let cfg = state.config.read().expect("read config");
    let manager = cfg.get_manager(&AppType::Claude).expect("claude manager");
    let p1_after = manager.providers.get("p1").expect("p1 exists");

    assert!(
        p1_after
            .settings_config
            .get("includeCoAuthoredBy")
            .is_none(),
        "common top-level keys should not be persisted into provider snapshot"
    );

    let env = p1_after
        .settings_config
        .get("env")
        .and_then(Value::as_object)
        .expect("provider env should be object");
    assert!(
        !env.contains_key("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC"),
        "common env keys should not be persisted into provider snapshot"
    );
    assert_eq!(
        env.get("ANTHROPIC_AUTH_TOKEN").and_then(Value::as_str),
        Some("token1"),
        "provider-specific env should remain in snapshot"
    );
}

#[test]
#[serial]
fn common_config_snippet_is_merged_into_codex_config_on_write() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::codex_config::get_codex_config_dir())
        .expect("create ~/.codex (initialized)");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);
    config.common_config_snippets.codex = Some("disable_response_storage = true".to_string());

    let state = state_from_config(config);

    let provider = Provider::with_id(
        "p1".to_string(),
        "First".to_string(),
        json!({
            "auth": { "OPENAI_API_KEY": "sk-test" },
            "config": "model_provider = \"first\"\nmodel = \"gpt-5.2-codex\"\n\n[model_providers.first]\nbase_url = \"https://api.example/v1\"\nwire_api = \"responses\"\nrequires_openai_auth = true\n"
        }),
        None,
    );

    ProviderService::add(&state, AppType::Codex, provider).expect("add should succeed");

    let live_text = std::fs::read_to_string(get_codex_config_path()).expect("read config.toml");
    assert!(
        live_text.contains("disable_response_storage = true"),
        "common snippet should be merged into config.toml"
    );
}

#[test]
#[serial]
fn codex_switch_extracts_common_snippet_preserving_mcp_servers() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "p1".to_string();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "First".to_string(),
                json!({ "config": "model_provider = \"first\"\nmodel = \"gpt-4\"\n\n[model_providers.first]\nbase_url = \"https://api.one.example/v1\"\n" }),
                None,
            ),
        );
        manager.providers.insert(
            "p2".to_string(),
            Provider::with_id(
                "p2".to_string(),
                "Second".to_string(),
                json!({ "config": "model_provider = \"second\"\nmodel = \"gpt-4\"\n\n[model_providers.second]\nbase_url = \"https://api.two.example/v1\"\n" }),
                None,
            ),
        );
    }

    let state = state_from_config(config);

    let config_toml = r#"model_provider = "azure"
model = "gpt-4"
disable_response_storage = true

[model_providers.azure]
name = "Azure OpenAI"
base_url = "https://azure.example/v1"
wire_api = "responses"

[mcp_servers.my_server]
base_url = "http://localhost:8080"
"#;

    let config_path = get_codex_config_path();
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent).expect("create codex dir");
    }
    std::fs::write(&config_path, config_toml).expect("seed config.toml");

    ProviderService::switch(&state, AppType::Codex, "p2").expect("switch should succeed");

    let cfg = state.config.read().expect("read config after switch");
    let extracted = cfg
        .common_config_snippets
        .codex
        .as_deref()
        .unwrap_or_default();

    assert!(
        extracted.contains("disable_response_storage = true"),
        "should keep top-level common config"
    );
    assert!(
        extracted.contains("[mcp_servers.my_server]"),
        "should keep mcp_servers table"
    );
    assert!(
        extracted.contains("base_url = \"http://localhost:8080\""),
        "should keep mcp_servers.* base_url"
    );
    assert!(
        !extracted
            .lines()
            .any(|line| line.trim_start().starts_with("model_provider")),
        "should remove top-level model_provider"
    );
    assert!(
        !extracted
            .lines()
            .any(|line| line.trim_start().starts_with("model =")),
        "should remove top-level model"
    );
    assert!(
        !extracted.contains("[model_providers"),
        "should remove entire model_providers table"
    );
}

#[test]
#[serial]
fn common_config_snippet_can_be_disabled_per_provider_for_codex() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::codex_config::get_codex_config_dir())
        .expect("create ~/.codex (initialized)");

    let config_path = get_codex_config_path();
    std::fs::write(
        &config_path,
        "disable_response_storage = true\nnetwork_access = \"restricted\"\n",
    )
    .expect("seed config.toml");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Codex);
    config.common_config_snippets.codex = Some("disable_response_storage = true".to_string());
    {
        let manager = config
            .get_manager_mut(&AppType::Codex)
            .expect("codex manager");
        manager.current = "p1".to_string();
        manager.providers.insert(
            "p1".to_string(),
            Provider::with_id(
                "p1".to_string(),
                "First".to_string(),
                json!({ "config": "model_provider = \"first\"\nmodel = \"gpt-4\"\n\n[model_providers.first]\nbase_url = \"https://api.one.example/v1\"\n" }),
                None,
            ),
        );
        manager.providers.insert(
            "p2".to_string(),
            serde_json::from_value(json!({
                "id": "p2",
                "name": "Second",
                "settingsConfig": { "config": "model_provider = \"second\"\nmodel = \"gpt-4\"\n\n[model_providers.second]\nbase_url = \"https://api.two.example/v1\"\n" },
                "meta": { "applyCommonConfig": false }
            }))
            .expect("parse provider p2"),
        );
    }

    let state = state_from_config(config);

    ProviderService::switch(&state, AppType::Codex, "p2").expect("switch should succeed");

    let live_text = std::fs::read_to_string(get_codex_config_path()).expect("read config.toml");
    assert!(
        !live_text.contains("disable_response_storage = true"),
        "common snippet should not be merged when applyCommonConfig=false"
    );
    assert!(
        live_text.contains("base_url = \"https://api.two.example/v1\""),
        "provider-specific config should be written"
    );
}

#[test]
fn extract_credentials_returns_expected_values() {
    let provider = Provider::with_id(
        "claude".into(),
        "Claude".into(),
        json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token",
                "ANTHROPIC_BASE_URL": "https://claude.example"
            }
        }),
        None,
    );
    let (api_key, base_url) =
        ProviderService::extract_credentials(&provider, &AppType::Claude).unwrap();
    assert_eq!(api_key, "token");
    assert_eq!(base_url, "https://claude.example");
}

#[test]
fn resolve_usage_script_credentials_falls_back_to_provider_values() {
    let provider = Provider::with_id(
        "claude".into(),
        "Claude".into(),
        json!({
            "env": {
                "ANTHROPIC_AUTH_TOKEN": "token",
                "ANTHROPIC_BASE_URL": "https://claude.example"
            }
        }),
        None,
    );
    let usage_script = crate::provider::UsageScript {
        enabled: true,
        language: "javascript".to_string(),
        code: String::new(),
        timeout: None,
        api_key: None,
        base_url: None,
        access_token: None,
        user_id: None,
        template_type: None,
        auto_query_interval: None,
    };

    let (api_key, base_url) = ProviderService::resolve_usage_script_credentials(
        &provider,
        &AppType::Claude,
        &usage_script,
    )
    .expect("should resolve via provider values");
    assert_eq!(api_key, "token");
    assert_eq!(base_url, "https://claude.example");
}

#[test]
fn resolve_usage_script_credentials_does_not_require_provider_api_key_when_script_has_one() {
    let provider = Provider::with_id(
        "claude".into(),
        "Claude".into(),
        json!({
            "env": {
                "ANTHROPIC_BASE_URL": "https://claude.example"
            }
        }),
        None,
    );
    let usage_script = crate::provider::UsageScript {
        enabled: true,
        language: "javascript".to_string(),
        code: String::new(),
        timeout: None,
        api_key: Some("override".to_string()),
        base_url: None,
        access_token: None,
        user_id: None,
        template_type: None,
        auto_query_interval: None,
    };

    let (api_key, base_url) = ProviderService::resolve_usage_script_credentials(
        &provider,
        &AppType::Claude,
        &usage_script,
    )
    .expect("should resolve base_url from provider without needing provider api key");
    assert_eq!(api_key, "override");
    assert_eq!(base_url, "https://claude.example");
}

#[test]
#[serial]
fn common_config_snippet_is_merged_into_gemini_env_on_write() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());
    std::fs::create_dir_all(crate::gemini_config::get_gemini_dir())
        .expect("create ~/.gemini (initialized)");

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Gemini);
    config.common_config_snippets.gemini =
        Some(r#"{"env":{"CC_SWITCH_GEMINI_COMMON":"1"}}"#.to_string());

    let state = state_from_config(config);

    let provider = Provider::with_id(
        "p1".to_string(),
        "First".to_string(),
        json!({
            "env": {
                "GEMINI_API_KEY": "token"
            }
        }),
        None,
    );

    ProviderService::add(&state, AppType::Gemini, provider).expect("add should succeed");

    let env = crate::gemini_config::read_gemini_env().expect("read gemini env");
    assert_eq!(
        env.get("CC_SWITCH_GEMINI_COMMON").map(String::as_str),
        Some("1"),
        "common snippet env key should be present in ~/.gemini/.env"
    );
    assert_eq!(
        env.get("GEMINI_API_KEY").map(String::as_str),
        Some("token"),
        "provider env key should remain in ~/.gemini/.env"
    );
}

#[test]
#[serial]
fn common_config_snippet_is_not_persisted_into_gemini_provider_snapshot_on_switch() {
    let temp_home = TempDir::new().expect("create temp home");
    let _env = EnvGuard::set_home(temp_home.path());

    let mut config = MultiAppConfig::default();
    config.ensure_app(&AppType::Gemini);
    config.common_config_snippets.gemini =
        Some(r#"{"env":{"CC_SWITCH_GEMINI_COMMON":"1"}}"#.to_string());

    let state = state_from_config(config);

    let p1 = Provider::with_id(
        "p1".to_string(),
        "First".to_string(),
        json!({
            "env": {
                "GEMINI_API_KEY": "token1"
            }
        }),
        None,
    );
    let p2 = Provider::with_id(
        "p2".to_string(),
        "Second".to_string(),
        json!({
            "env": {
                "GEMINI_API_KEY": "token2"
            }
        }),
        None,
    );

    ProviderService::add(&state, AppType::Gemini, p1).expect("add p1");
    ProviderService::add(&state, AppType::Gemini, p2).expect("add p2");

    ProviderService::switch(&state, AppType::Gemini, "p2").expect("switch to p2");

    let cfg = state.config.read().expect("read config");
    let manager = cfg.get_manager(&AppType::Gemini).expect("gemini manager");
    let p1_after = manager.providers.get("p1").expect("p1 exists");

    let env = p1_after
        .settings_config
        .get("env")
        .and_then(Value::as_object)
        .expect("provider env should be object");

    assert!(
        !env.contains_key("CC_SWITCH_GEMINI_COMMON"),
        "common env keys should not be persisted into provider snapshot"
    );
    assert_eq!(
        env.get("GEMINI_API_KEY").and_then(Value::as_str),
        Some("token1"),
        "provider-specific env should remain in snapshot"
    );
}
