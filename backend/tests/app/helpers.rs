use backend::{conf, startup::Application, telemetry};
use hyper::StatusCode;
use once_cell::sync::Lazy;
use static_routes::*;

static TRACING: Lazy<()> = Lazy::new(|| {
    let subscriber = telemetry::TracingSubscriber::new("testing");

    if std::env::var("TEST_LOG").is_ok() {
        telemetry::init_global_default(subscriber.build(std::io::stdout));
    } else {
        telemetry::init_global_default(subscriber.build(std::io::sink));
    };
});

pub async fn spawn_app() -> TestApp {
    Lazy::force(&TRACING);

    let env_conf = conf::EnvConf::test_default();
    let env = conf::Env::Local;
    let conf = conf::Conf { env, env_conf };

    let application = Application::build(&conf).await;

    let host = application.host();
    let port = application.port();

    let address = format!("http://{}:{}", host, port);

    let _ = tokio::spawn(application.server());

    let api_client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .cookie_store(true)
        .build()
        .unwrap();

    TestApp {
        address,
        port,
        api_client,
    }
}

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub api_client: reqwest::Client,
}

impl TestApp {
    pub fn get(&self, static_path: impl Get) -> reqwest::RequestBuilder {
        self.api_client
            .get(static_path.get().with_base(&self.address).complete())
    }

    #[allow(unused)]
    pub fn post(&self, static_path: impl Post) -> reqwest::RequestBuilder {
        self.api_client
            .post(static_path.post().with_base(&self.address).complete())
    }
}

#[allow(unused)]
pub fn assert_is_redirect_to(response: &reqwest::Response, location: impl Get) {
    assert_eq!(StatusCode::SEE_OTHER, response.status());
    assert_eq!(
        location.get().complete(),
        response.headers().get("Location").unwrap()
    );
}
