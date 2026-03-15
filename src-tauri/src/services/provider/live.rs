use std::collections::HashMap;

use serde_json::Value;

use crate::app_config::AppType;
use crate::codex_config::{get_codex_auth_path, get_codex_config_path};
use crate::config::{delete_file, get_claude_settings_path, read_json_file, write_json_file};
use crate::error::AppError;
use crate::provider::Provider;
use crate::store::AppState;

#[derive(Clone)]
pub(super) enum LiveSnapshot {
    Claude {
        settings: Option<Value>,
    },
    Codex {
        auth: Option<Value>,
        config: Option<String>,
    },
    Gemini {
        env: Option<HashMap<String, String>>,
        config: Option<Value>,
    },
    OpenCode {
        config: Option<Value>,
    },
    OpenClaw {
        config_source: Option<String>,
    },
}

impl LiveSnapshot {
    pub(super) fn restore(&self) -> Result<(), AppError> {
        match self {
            LiveSnapshot::Claude { settings } => {
                let path = get_claude_settings_path();
                if let Some(value) = settings {
                    write_json_file(&path, value)?;
                } else if path.exists() {
                    delete_file(&path)?;
                }
            }
            LiveSnapshot::Codex { auth, config } => {
                let auth_path = get_codex_auth_path();
                let config_path = get_codex_config_path();
                if let Some(value) = auth {
                    write_json_file(&auth_path, value)?;
                } else if auth_path.exists() {
                    delete_file(&auth_path)?;
                }

                if let Some(text) = config {
                    crate::config::write_text_file(&config_path, text)?;
                } else if config_path.exists() {
                    delete_file(&config_path)?;
                }
            }
            LiveSnapshot::Gemini { env, config } => {
                use crate::gemini_config::{
                    get_gemini_env_path, get_gemini_settings_path, write_gemini_env_atomic,
                };

                let path = get_gemini_env_path();
                if let Some(env_map) = env {
                    write_gemini_env_atomic(env_map)?;
                } else if path.exists() {
                    delete_file(&path)?;
                }

                let settings_path = get_gemini_settings_path();
                match config {
                    Some(cfg) => {
                        write_json_file(&settings_path, cfg)?;
                    }
                    None if settings_path.exists() => {
                        delete_file(&settings_path)?;
                    }
                    _ => {}
                }
            }
            LiveSnapshot::OpenCode { config } => {
                let path = crate::opencode_config::get_opencode_config_path();
                if let Some(value) = config {
                    write_json_file(&path, value)?;
                } else if path.exists() {
                    delete_file(&path)?;
                }
            }
            LiveSnapshot::OpenClaw { config_source } => {
                let path = crate::openclaw_config::get_openclaw_config_path();
                if let Some(source) = config_source {
                    crate::openclaw_config::write_openclaw_config_source(source)?;
                } else if path.exists() {
                    delete_file(&path)?;
                }
            }
        }
        Ok(())
    }
}

pub(super) fn capture_live_snapshot(app_type: &AppType) -> Result<LiveSnapshot, AppError> {
    match app_type {
        AppType::Claude => {
            let path = get_claude_settings_path();
            let settings = if path.exists() {
                Some(read_json_file(&path)?)
            } else {
                None
            };
            Ok(LiveSnapshot::Claude { settings })
        }
        AppType::Codex => {
            let auth_path = get_codex_auth_path();
            let config_path = get_codex_config_path();
            let auth = if auth_path.exists() {
                Some(read_json_file(&auth_path)?)
            } else {
                None
            };
            let config = if config_path.exists() {
                Some(crate::codex_config::read_and_validate_codex_config_text()?)
            } else {
                None
            };
            Ok(LiveSnapshot::Codex { auth, config })
        }
        AppType::Gemini => {
            use crate::gemini_config::{
                get_gemini_env_path, get_gemini_settings_path, read_gemini_env,
            };

            let env_path = get_gemini_env_path();
            let env = if env_path.exists() {
                Some(read_gemini_env()?)
            } else {
                None
            };
            let settings_path = get_gemini_settings_path();
            let config = if settings_path.exists() {
                Some(read_json_file(&settings_path)?)
            } else {
                None
            };
            Ok(LiveSnapshot::Gemini { env, config })
        }
        AppType::OpenCode => {
            let path = crate::opencode_config::get_opencode_config_path();
            let config = if path.exists() {
                Some(crate::opencode_config::read_opencode_config()?)
            } else {
                None
            };
            Ok(LiveSnapshot::OpenCode { config })
        }
        AppType::OpenClaw => {
            let config_source = crate::openclaw_config::read_openclaw_config_source()?;
            Ok(LiveSnapshot::OpenClaw { config_source })
        }
    }
}

pub fn import_openclaw_providers_from_live(state: &AppState) -> Result<usize, AppError> {
    let providers = crate::openclaw_config::get_typed_providers()?;
    if providers.is_empty() {
        return Ok(0);
    }

    let mut imported = 0;
    {
        let mut config = state.config.write().map_err(AppError::from)?;
        config.ensure_app(&AppType::OpenClaw);
        let manager = config
            .get_manager_mut(&AppType::OpenClaw)
            .ok_or_else(|| AppError::Config("OpenClaw manager missing".to_string()))?;

        for (id, provider_config) in providers {
            if id.trim().is_empty() || provider_config.models.is_empty() {
                continue;
            }
            if manager.providers.contains_key(&id) {
                continue;
            }

            let name = provider_config
                .models
                .first()
                .and_then(|model| model.name.clone())
                .unwrap_or_else(|| id.clone());
            let settings_config = serde_json::to_value(&provider_config)
                .map_err(|source| AppError::JsonSerialize { source })?;

            manager.providers.insert(
                id.clone(),
                Provider::with_id(id, name, settings_config, None),
            );
            imported += 1;
        }
    }

    if imported > 0 {
        state.save()?;
    }

    Ok(imported)
}
