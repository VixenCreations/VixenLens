use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Config {
    pub feature_flags: FeatureFlags,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FeatureFlags {
    pub update_db_when_startup: bool,
    pub language: String,
}

fn default_config() -> Config {
    Config {
        feature_flags: FeatureFlags {
            update_db_when_startup: false,
            language: "ja".to_string(),
        },
    }
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
		let base = app
				.path()
				.local_data_dir()
				.map_err(|e| format!("local_data_dir unavailable: {e}"))?;

    Ok(base.join("VRCX PhotoSearch").join("config"))
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("config.json"))
}

fn load_config_from_disk(app: &AppHandle) -> Result<Config, String> {
    let path = config_path(app)?;
    let dir = path.parent().unwrap();

    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;

    if !path.exists() {
        let default = default_config();
        save_config_to_disk(app, &default)?;
        println!("[config] created default {}", path.display());
        return Ok(default);
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read config: {e}"))?;

    let cfg =
        serde_json::from_str(&content).map_err(|e| format!("Parse config failed: {e}"))?;

    println!("[config] loaded {}", path.display());
    Ok(cfg)
}

fn save_config_to_disk(app: &AppHandle, config: &Config) -> Result<(), String> {
    let path = config_path(app)?;
    let dir = path.parent().unwrap();

    fs::create_dir_all(dir)
        .map_err(|e| format!("Failed to create config dir: {e}"))?;

    let content =
        serde_json::to_string_pretty(config).map_err(|e| format!("Serialize failed: {e}"))?;

    fs::write(&path, content).map_err(|e| format!("Write failed: {e}"))?;

    println!("[config] saved {}", path.display());
    Ok(())
}

#[tauri::command]
pub fn get_config(app: AppHandle) -> Result<Config, String> {
    load_config_from_disk(&app)
}

#[tauri::command]
pub fn set_config(app: AppHandle, config: Config) -> Result<(), String> {
    save_config_to_disk(&app, &config)
}
