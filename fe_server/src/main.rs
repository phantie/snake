// Server instantiation
//

mod conf;
mod serve_files;
mod server;
mod trace;

use server::{Application, ServerOutput};

#[tokio::main]
async fn main() -> ServerOutput {
    let env = conf::Env::derive();
    let env_conf = conf::EnvConf::derive(env);

    trace::TracingSubscriber::new()
        .pretty(env_conf.log.pretty)
        .set_global_default();

    tracing::debug!("Env: {}", env);
    tracing::debug!("{:?}", env_conf);

    let conf = conf::Conf::new(env, env_conf);

    let app = Application::build(conf).await;

    app.server().await
}
