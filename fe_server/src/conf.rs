// Configuration definitions, functions and tests
//
// TODO fix TestApp::spawn_app() requiring DIR env var

use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string as de_num;

#[cfg(not(test))]
lazy_static::lazy_static! {
    static ref ENV_CONF: EnvConf = EnvConf::derive();
    static ref ENV: Env = Env::derive();
}

static ENV_PREFIX: &str = "FE_SRV";

fn prefixed_env(suffix: &str) -> String {
    format!("{}__{}", ENV_PREFIX, suffix)
}

#[derive(Clone)]
pub struct Conf {
    pub env_conf: EnvConf,
    pub env: Env,
}

#[derive(Deserialize, Debug, Clone)]
pub struct EnvConf {
    #[serde(deserialize_with = "de_num")]
    pub port: u16,
    pub host: String,
    pub dir: String,
    pub fallback: Option<String>,
    #[serde(deserialize_with = "de_num")]
    pub request_path_lru_size: std::num::NonZeroUsize,
    pub log: Log,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Log {
    pub pretty: bool,
}

impl EnvConf {
    pub fn derive() -> Self {
        fn join_filename(conf_dir: &std::path::PathBuf, filename: &str) -> String {
            conf_dir
                .join(filename)
                .into_os_string()
                .into_string()
                .unwrap()
        }

        let conf_dir = std::env::var(prefixed_env("CONF_DIR"))
            .map(|v| std::path::PathBuf::from(v))
            .unwrap_or_else(|_| {
                let base_path = std::env::current_dir().unwrap();
                base_path.join("conf")
            });

        let conf_builder = config::Config::builder()
            .add_source(
                config::File::with_name(&join_filename(&conf_dir, "default")).required(true),
            )
            .add_source(
                config::File::with_name(&join_filename(&conf_dir, Env::current().as_ref()))
                    .required(false),
            )
            .add_source(config::Environment::with_prefix(ENV_PREFIX).separator("__"))
            .build();

        let conf = conf_builder.unwrap();

        match conf.try_deserialize() {
            Ok(conf) => conf,
            Err(e) => {
                dbg!(&e);
                Err(e).expect("correct config")
            }
        }
    }

    #[cfg(not(test))]
    pub fn current() -> &'static Self {
        &ENV_CONF
    }

    #[cfg(test)]
    pub fn current() -> Self {
        Self::derive()
    }

    #[allow(unused)] // RA bug
    pub fn test_default() -> Self {
        Self {
            port: 0,
            dir: "".to_string(), // TODO
            fallback: None,
            request_path_lru_size: std::num::NonZeroUsize::new(30).unwrap(),
            host: "127.0.0.1".into(),
            log: Log { pretty: false },
        }
    }
}

use derive_more::Display;

#[derive(Debug, PartialEq, Display, Clone, Copy)]
pub enum Env {
    #[display(fmt = "local")]
    Local,
    #[display(fmt = "prod")]
    Prod,
}

impl Env {
    fn derive() -> Self {
        // One variable to rule all
        let glob_env = std::env::var("SNK_ENV").unwrap_or_else(|_| "local".into());

        // Or set a more specific per executable
        std::env::var(prefixed_env("ENV"))
            .unwrap_or(glob_env)
            .try_into()
            .expect("valid variable")
    }

    #[cfg(not(test))]
    pub fn current() -> Self {
        *ENV
    }

    #[cfg(test)]
    pub fn current() -> Self {
        Self::derive()
    }

    #[allow(unused)]
    pub fn local(&self) -> bool {
        matches!(self, Self::Local)
    }

    #[allow(unused)]
    pub fn prod(&self) -> bool {
        matches!(self, Self::Prod)
    }
}

impl AsRef<str> for Env {
    fn as_ref(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::Prod => "prod",
        }
    }
}

impl TryFrom<String> for Env {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "prod" => Ok(Self::Prod),
            other => Err(format!(
                "{} is not a supported environment. Use either `local` or `prod`.",
                other
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use envtestkit::{lock::lock_test, set_env};

    #[test]
    fn default_current_env() {
        assert!(Env::current().local());
    }

    #[test]
    fn default_current_env_not() {
        assert!(!Env::current().prod());
    }

    #[test]
    fn env() {
        #[derive(Debug)]
        struct Test<'a> {
            glob_env: Option<&'a str>,
            local_env: Option<&'a str>,
            result: Result<Env, ()>,
        }

        impl<'a> Test<'a> {
            fn run(&self) {
                let _lock = lock_test();

                let _1 = self.glob_env.map(|env| set_env("SNK_ENV".into(), env));

                let _2 = self
                    .local_env
                    .map(|env| set_env(prefixed_env("ENV").into(), env));

                match &self.result {
                    #[allow(unused)]
                    Ok(expected) => {
                        assert_eq!(&Env::current(), expected, "{:?}", self);
                    }
                    Err(()) => {
                        let result = std::panic::catch_unwind(|| Env::current());
                        assert!(result.is_err(), "{:?}", self);
                    }
                }
            }
        }

        // Successful cases
        {
            Test {
                glob_env: Some(Env::Prod.as_ref()),
                local_env: None,
                result: Ok(Env::Prod),
            }
            .run();

            Test {
                glob_env: Some(Env::Local.as_ref()),
                local_env: None,
                result: Ok(Env::Local),
            }
            .run();

            Test {
                glob_env: None,
                local_env: None,
                result: Ok(Env::Local),
            }
            .run();

            Test {
                glob_env: None,
                local_env: Some(Env::Local.as_ref()),
                result: Ok(Env::Local),
            }
            .run();

            Test {
                glob_env: None,
                local_env: Some(Env::Prod.as_ref()),
                result: Ok(Env::Prod),
            }
            .run();

            Test {
                glob_env: Some(Env::Local.as_ref()),
                local_env: Some(Env::Local.as_ref()),
                result: Ok(Env::Local),
            }
            .run();

            Test {
                glob_env: Some(Env::Local.as_ref()),
                local_env: Some(Env::Prod.as_ref()),
                result: Ok(Env::Prod),
            }
            .run();

            Test {
                glob_env: Some(Env::Prod.as_ref()),
                local_env: Some(Env::Local.as_ref()),
                result: Ok(Env::Local),
            }
            .run();

            Test {
                glob_env: Some(Env::Prod.as_ref()),
                local_env: Some(Env::Prod.as_ref()),
                result: Ok(Env::Prod),
            }
            .run();
        }

        // Unsuccessful cases
        {
            let invalid_env_value = "";

            Test {
                glob_env: Some(invalid_env_value),
                local_env: None,
                result: Err(()),
            }
            .run();

            Test {
                glob_env: Some(invalid_env_value),
                local_env: None,
                result: Err(()),
            }
            .run();

            Test {
                glob_env: Some(invalid_env_value),
                local_env: Some(invalid_env_value),
                result: Err(()),
            }
            .run();
        }
    }
}
