// Configuration definitions, functions and tests
//

use serde::Deserialize;
use serde_aux::field_attributes::deserialize_number_from_string;

static ENV_PREFIX: &str = "BE";

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
    #[serde(deserialize_with = "deserialize_number_from_string")]
    pub port: u16,
    pub host: String,
    pub log: Log,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Log {
    pub pretty: bool,
}

impl EnvConf {
    pub fn current() -> Self {
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

    #[allow(unused)] // RA bug
    pub fn test_default() -> Self {
        Self {
            port: 0,
            host: "127.0.0.1".into(),
            log: Log { pretty: false },
        }
    }
}

use derive_more::Display;

#[derive(Debug, PartialEq, Display, Clone)]
pub enum Env {
    #[display(fmt = "local")]
    Local,
    #[display(fmt = "prod")]
    Prod,
}

#[allow(unused)]
impl Env {
    pub fn current() -> Self {
        // One variable to rule all
        let hort_env = std::env::var("HORT_ENV").unwrap_or_else(|_| "local".into());

        // Or set a more specific per executable
        std::env::var(prefixed_env("ENV"))
            .unwrap_or(hort_env)
            .try_into()
            .expect("valid variable")
    }

    pub fn local(&self) -> bool {
        matches!(self, Self::Local)
    }

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
            hort_env: Option<&'a str>,
            local_env: Option<&'a str>,
            result: Result<Env, ()>,
        }

        impl<'a> Test<'a> {
            fn run(&self) {
                let _lock = lock_test();

                let _1 = self.hort_env.map(|env| set_env("HORT_ENV".into(), env));

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
                hort_env: Some(Env::Prod.as_ref()),
                local_env: None,
                result: Ok(Env::Prod),
            }
            .run();

            Test {
                hort_env: Some(Env::Local.as_ref()),
                local_env: None,
                result: Ok(Env::Local),
            }
            .run();

            Test {
                hort_env: None,
                local_env: None,
                result: Ok(Env::Local),
            }
            .run();

            Test {
                hort_env: None,
                local_env: Some(Env::Local.as_ref()),
                result: Ok(Env::Local),
            }
            .run();

            Test {
                hort_env: None,
                local_env: Some(Env::Prod.as_ref()),
                result: Ok(Env::Prod),
            }
            .run();

            Test {
                hort_env: Some(Env::Local.as_ref()),
                local_env: Some(Env::Local.as_ref()),
                result: Ok(Env::Local),
            }
            .run();

            Test {
                hort_env: Some(Env::Local.as_ref()),
                local_env: Some(Env::Prod.as_ref()),
                result: Ok(Env::Prod),
            }
            .run();

            Test {
                hort_env: Some(Env::Prod.as_ref()),
                local_env: Some(Env::Local.as_ref()),
                result: Ok(Env::Local),
            }
            .run();

            Test {
                hort_env: Some(Env::Prod.as_ref()),
                local_env: Some(Env::Prod.as_ref()),
                result: Ok(Env::Prod),
            }
            .run();
        }

        // Unsuccessful cases
        {
            let invalid_env_value = "";

            Test {
                hort_env: Some(invalid_env_value),
                local_env: None,
                result: Err(()),
            }
            .run();

            Test {
                hort_env: Some(invalid_env_value),
                local_env: None,
                result: Err(()),
            }
            .run();

            Test {
                hort_env: Some(invalid_env_value),
                local_env: Some(invalid_env_value),
                result: Err(()),
            }
            .run();
        }
    }
}
