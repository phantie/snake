use backend::conf;
use backend::startup::Application;
use backend::telemetry;

#[tokio::main]
async fn main() -> hyper::Result<()> {
    let subscriber = telemetry::TracingSubscriber::new("site").build(std::io::stdout);
    telemetry::init_global_default(subscriber);

    let env = conf::Env::current();

    let env_conf = conf::EnvConf::current();

    let conf = conf::Conf {
        env: env.clone(),
        env_conf: env_conf.clone(),
    };

    tracing::debug!("Env: {}", env);

    let application = Application::build(&conf).await;

    application.server().await
}
