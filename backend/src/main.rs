use backend::conf::{self, env_conf, get_env};
use backend::startup::Application;
use backend::telemetry;

#[tokio::main]
async fn main() -> hyper::Result<()> {
    let subscriber = telemetry::TracingSubscriber::new("site").build(std::io::stdout);
    telemetry::init_global_default(subscriber);

    let env_conf = env_conf();

    tracing::info!("APP_ENVIRONMENT={}", get_env().as_str());

    let conf = conf::Conf { env: env_conf };

    let application = Application::build(&conf).await;

    application.server().await
}
