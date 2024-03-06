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
    use tower_http::add_extension::AddExtensionLayer;

    mod routes {
        pub mod fallback {
            use std::io::Read;

            use axum::{body::HttpBody, response::IntoResponse, Extension};

            use crate::serve_files::*;

            pub async fn fallback(
                uri: axum::http::Uri,
                Extension(cache): Extension<crate::serve_files::Cache>,
            ) -> axum::response::Response {
                let request_path = {
                    let request_path = uri.to_string();
                    request_path.trim_start_matches('/').trim().to_string()
                };

                use crate::conf;

                let conf = conf::EnvConf::current();

                if let Some(file) = cache.get_request_path(&request_path).await {
                    tracing::info!("cache hit for request path: {request_path:?}");
                    return file_response(&file);
                }

                let dir = std::path::Path::new(&conf.dir);
                let file_path = dir.join(request_path.clone());

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

                let file = cache.get_disk_path(&file_path).await;

                #[allow(unused)]
                let display_cache_keys = async {
                    let lock = cache.lock().await;
                    let keys = lock.iter().map(|(k, _)| k).collect::<Vec<_>>();
                    tracing::warn!("cache keys: {keys:?}");
                };

                match file {
                    None => {
                        tracing::warn!("cache miss on file path: {file_path:?}");
                        let process_file = |mut file: std::fs::File| {
                            let modified = file.metadata().unwrap().modified().unwrap();
                            let mut contents = vec![];
                            file.read_to_end(&mut contents);
                            File {
                                contents,
                                path: Box::new(file_path.clone()),
                                request_path: request_path.clone(),
                                modified,
                            }
                        };

                        let mut file = std::fs::File::open(&file_path).expect("opens when exists");
                        let file = process_file(file);
                        let response = file_response(&file);
                        cache.insert(request_path, std::sync::Arc::new(file)).await;
                        // display_cache_keys.await;
                        response
                    }
                    Some(cached) => {
                        tracing::warn!("cache hit on file path: {file_path:?}");
                        // do not go to disk, reuse cached value
                        cache.insert(request_path, cached.clone()).await;
                        // display_cache_keys.await;
                        file_response(&cached)
                    }
                }
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
            .layer(AddExtensionLayer::new(crate::serve_files::Cache::default()))
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
