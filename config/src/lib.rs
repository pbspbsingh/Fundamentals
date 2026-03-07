use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub chrome_path: PathBuf,
    pub user_data_dir: PathBuf,
    pub chrome_args: Vec<String>,
    pub launch_if_needed: bool,

    pub rust_log: String,
}

static CONFIG: OnceLock<Config> = OnceLock::new();

pub fn config() -> &'static Config {
    CONFIG.get_or_init(|| {
        let content = std::fs::read_to_string("config.toml").expect("Failed to read config.toml");
        toml::from_str(&content).expect("Failed to parse config.toml")
    })
}
