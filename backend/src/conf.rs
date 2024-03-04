use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string; // to deserialize variables provided via env vars

static ENV_PREFIX: &str = "GS_BE";

fn prefixed_env(suffix: &str) -> String {
    format!("{}__{}", ENV_PREFIX, suffix)
}

#[derive(Deserialize)]
pub struct EnvConf {
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,

    pub features: EnvFeatures,
}

impl EnvConf {
    pub fn test() -> Self {
        Self {
            host: "localhost".into(),
            port: 0,
            features: EnvFeatures {},
        }
    }
}

#[derive(Deserialize, Clone)]
pub struct EnvFeatures {}

pub fn get_env() -> Environment {
    std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".into())
        .try_into()
        .expect("Failed to parse APP_ENVIRONMENT.")
}

pub struct Conf {
    pub env: EnvConf,
}

pub fn env_conf() -> EnvConf {
    fn conf_path(conf_dir: &std::path::PathBuf, filename: &str) -> String {
        conf_dir
            .join(filename)
            .into_os_string()
            .into_string()
            .unwrap()
    }

    let base_path = std::env::current_dir().expect("Failed to determine the current directory");

    let configuration_directory = base_path.join("conf");
    let env = get_env();

    let config_builder = config::Config::builder()
        .add_source(
            config::File::with_name(&conf_path(&configuration_directory, "base")).required(true),
        )
        .add_source(
            config::File::with_name(&conf_path(&configuration_directory, env.as_str()))
                .required(true),
        )
        .add_source(config::Environment::with_prefix("app").separator("__"))
        .build();

    let config = config_builder.unwrap();
    let _config_clone = config.clone();

    match config.try_deserialize() {
        Ok(settings) => settings,
        Err(e) => Err(e).unwrap(),
    }
}

pub enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }

    pub fn local(&self) -> bool {
        matches!(self, Self::Local)
    }

    pub fn prod(&self) -> bool {
        matches!(self, Self::Production)
    }
}

impl TryFrom<String> for Environment {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `production`.",
                other
            )),
        }
    }
}
