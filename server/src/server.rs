// Server and router definition and tests
//

#![allow(unused)]

use crate::conf;
use hyper::StatusCode;
use std::sync::Arc;

type ServerOutput = hyper::Result<()>;
type Server = std::pin::Pin<Box<dyn std::future::Future<Output = ServerOutput> + Send>>;

pub struct AppState {
    conf: conf::Conf,
}

pub struct Application {
    host: String,
    port: u16,
    server: Server,
}

impl Application {
    pub async fn build(conf: &conf::Conf) -> Self {
        let address = format!("{}:{}", conf.env_conf.host, conf.env_conf.port);
        tracing::debug!("Binding to {}", address);
        let listener = std::net::TcpListener::bind(&address).expect("vacant port");
        let host = conf.env_conf.host.clone();
        let port = listener.local_addr().unwrap().port();
        let address = format!("{}:{}", host, port);
        tracing::info!("Serving on http://{}", address);

        let app_state = Arc::new(AppState { conf: conf.clone() });

        return Self {
            server: Box::pin(
                axum::Server::from_tcp(listener).unwrap().serve(
                    routing::router(&conf)
                        .with_state(app_state)
                        .into_make_service(),
                ),
            ),
            port,
            host,
        };
    }

    pub fn server(self) -> Server {
        self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn host(&self) -> &str {
        &self.host
    }
}

mod routing {
    use super::*;
    use axum::routing::get;
    use axum::routing::Router;

    mod routes {
        pub mod fallback {
            use std::io::Read;

            use axum::response::IntoResponse;

            use crate::serve_files::*;

            pub async fn fallback(uri: axum::http::Uri) -> axum::response::Response {
                let relative_file_path = {
                    let relative_file_path = uri.to_string();
                    relative_file_path
                        .trim_start_matches('/')
                        .trim()
                        .to_string()
                };

                use crate::conf;

                let conf = conf::EnvConf::current();

                let dir = std::path::Path::new(&conf.dir);
                let file_path = dir.join(relative_file_path);

                let (file_path) = if file_path.is_file() {
                    file_path
                } else {
                    match &conf.fallback {
                        Some(file_path) => {
                            let file_path = std::path::Path::new(file_path);

                            if file_path.is_file() {
                                file_path.to_path_buf()
                            } else {
                                return hyper::StatusCode::INTERNAL_SERVER_ERROR.into_response();
                            }
                        }
                        None => return hyper::StatusCode::NOT_FOUND.into_response(),
                    }
                };

                let mut file = std::fs::File::open(&file_path).expect("opens when exists");

                // tracing::info!("sending file {:?}", file_path);

                let modified = file.metadata().unwrap().modified().unwrap();
                let mut contents = vec![];
                file.read_to_end(&mut contents);

                file_response(contents, file_path, modified)
            }
        }

        use super::*;
        pub async fn health() -> StatusCode {
            StatusCode::OK
        }
    }

    pub fn router(conf: &conf::Conf) -> Router<Arc<AppState>> {
        let api_router = axum::Router::new().route("/health", get(routes::health));

        Router::new()
            .nest("/api", api_router)
            .fallback(routes::fallback::fallback)
            .layer(crate::trace::request_trace_layer())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub struct TestApp {
        pub port: u16,
        pub address: String,

        app_handle: tokio::task::JoinHandle<ServerOutput>,
    }

    impl TestApp {
        async fn spawn() -> Self {
            let env_conf = conf::EnvConf::test_default();
            let env = conf::Env::Local;
            let conf = conf::Conf { env, env_conf };

            let app = Application::build(&conf).await;
            let port = app.port();
            let address = format!("http://{}:{}", app.host(), port);
            let app_handle = tokio::spawn(app.server());

            Self {
                port,
                address,
                app_handle,
            }
        }
    }

    impl Drop for TestApp {
        fn drop(&mut self) {
            self.app_handle.abort();
        }
    }

    #[tokio::test]
    async fn spawn_app() {
        let app = TestApp::spawn().await;
    }

    #[tokio::test]
    async fn health() {
        let app = TestApp::spawn().await;

        let r = reqwest::get(format!("{}/api/health", app.address))
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }
}
