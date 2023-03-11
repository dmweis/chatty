use std::path::PathBuf;

use anyhow::{Context, Result};
use config::Config;
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "dmweis";
const PROJECT_APPLICATION_NAME: &str = "chatty";

const CHATTY_CLI_CONFIG_FILE_NAME: &str = "config";
const CHATTY_CLI_CONFIG_FILE_EXTENSION: &str = "yaml";

pub fn get_project_dirs() -> Result<ProjectDirs> {
    ProjectDirs::from(
        PROJECT_QUALIFIER,
        PROJECT_ORGANIZATION,
        PROJECT_APPLICATION_NAME,
    )
    .context("failed to establish project dirs")
}

fn get_config_file_path() -> Result<PathBuf> {
    let proj_dirs = get_project_dirs()?;
    let config_dir_path = proj_dirs.config_dir();
    Ok(config_dir_path.join(CHATTY_CLI_CONFIG_FILE_NAME))
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct AppConfig {
    pub open_ai_api_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mqtt: Option<MqttConfig>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            open_ai_api_key: String::from(
                "Get token from https://platform.openai.com/account/api-keys",
            ),
            mqtt: None,
        }
    }
}

impl AppConfig {
    pub fn load_dev_config() -> anyhow::Result<Self> {
        let settings = Config::builder()
            .add_source(config::File::with_name("configuration/settings"))
            .add_source(config::File::with_name("configuration/dev_settings").required(false))
            .add_source(config::Environment::with_prefix("APP"))
            .build()?;

        Ok(settings.try_deserialize::<AppConfig>()?)
    }

    pub fn load_user_config() -> anyhow::Result<Self> {
        let config_file_path = get_config_file_path()?;
        let settings = Config::builder()
            .add_source(config::File::from(config_file_path))
            .add_source(config::Environment::with_prefix("CHATTY"))
            .build()?;

        Ok(settings.try_deserialize::<AppConfig>()?)
    }

    pub fn save_user_config(&self) -> anyhow::Result<()> {
        let config_file_path =
            get_config_file_path()?.with_extension(CHATTY_CLI_CONFIG_FILE_EXTENSION);

        std::fs::create_dir_all(
            config_file_path
                .parent()
                .context("failed to get config file parent directory")?,
        )?;

        let file = std::fs::File::create(config_file_path)?;
        serde_yaml::to_writer(file, self)?;
        Ok(())
    }
}

// weird serde default thing
const DEFAULT_MQTT_PORT: u16 = 1883;

const fn default_mqtt_port() -> u16 {
    DEFAULT_MQTT_PORT
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct MqttConfig {
    pub broker_host: String,
    #[serde(default = "default_mqtt_port")]
    pub broker_port: u16,
    pub client_id: String,
}
