use config::Config;
use serde::Deserialize;

pub fn get_configuration() -> anyhow::Result<AppConfig> {
    let settings = Config::builder()
        .add_source(config::File::with_name("configuration/settings"))
        .add_source(config::File::with_name("configuration/dev_settings").required(false))
        .add_source(config::Environment::with_prefix("APP"))
        .build()?;

    Ok(settings.try_deserialize::<AppConfig>()?)
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub open_ai_api_key: String,
}
