use backend::conf;
use backend::startup::Application;
use backend::trace;

#[tokio::main]
async fn main() -> hyper::Result<()> {
    let env = conf::Env::current();

    let env_conf = conf::EnvConf::current();

    let conf = conf::Conf {
        env: env.clone(),
        env_conf: env_conf.clone(),
    };

    trace::TracingSubscriber::new()
        .pretty(env_conf.log.pretty)
        .set_global_default();

    tracing::debug!("Env: {}", env);
    tracing::debug!("{:?}", env_conf);

    let application = Application::build(&conf).await;

    application.server().await
}
