// Server and router definition and tests
//

use crate::conf;

pub type ServerOutput = hyper::Result<()>;
type Server = std::pin::Pin<Box<dyn std::future::Future<Output = ServerOutput> + Send>>;

pub struct Application {
    server: Server,
    host: String,
    port: u16,
}

impl Application {
    pub async fn build(conf: conf::Conf) -> Self {
        let address = format!("{}:{}", conf.host, conf.port);
        tracing::debug!("Binding to {}", address);
        let listener = std::net::TcpListener::bind(&address).expect("vacant port");
        let host = conf.host.clone();
        let port = listener.local_addr().unwrap().port();
        let address = format!("{}:{}", host, port);
        tracing::info!("Serving on http://{}", address);

        return Self {
            server: Box::pin(axum::Server::from_tcp(listener).unwrap().serve(
                routing::router(conf).into_make_service_with_connect_info::<UserConnectInfo>(),
            )),
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
    use crate::routes::*;
    use axum::routing::Router;
    #[allow(unused_imports)]
    use axum::routing::{get, post};
    #[allow(unused_imports)]
    use static_routes::{Get, Post};
    use tower_http::{add_extension::AddExtensionLayer, compression::CompressionLayer};

    mod routes {
        use hyper::StatusCode;

        pub async fn health() -> StatusCode {
            StatusCode::OK
        }
    }

    pub fn router(conf: crate::conf::Conf) -> Router {
        let routes = static_routes::routes().api;

        let api_router = Router::new()
            .route(routes.health_check.get().postfix(), get(routes::health))
            // TODO investigate why POST on /lobby gives 200
            .route("/snake/ws", get(snake_ws::ws));

        Router::new()
            .nest("/api", api_router)
            .layer(CompressionLayer::new())
            .layer(AddExtensionLayer::new(conf))
            .layer(AddExtensionLayer::new(
                crate::mp::lobby::lobbies::Lobbies::default(),
            ))
            .layer(AddExtensionLayer::new(
                crate::mp::lobby::usernames::PlayerUserNames::default(),
            ))
            .layer(crate::trace::request_trace_layer())
    }
}

#[derive(Clone, Debug)]
pub struct UserConnectInfo {
    remote_addr: std::net::SocketAddr,
}

impl UserConnectInfo {
    pub fn socket_addr(&self, headers: &hyper::HeaderMap) -> std::net::SocketAddr {
        let ip = ip_address(headers);
        let mut sock = self.remote_addr;
        // rewrite ip address because server may be behind reverse proxy
        sock.set_ip(ip);
        sock
    }
}

impl axum::extract::connect_info::Connected<&hyper::server::conn::AddrStream> for UserConnectInfo {
    fn connect_info(target: &hyper::server::conn::AddrStream) -> Self {
        Self {
            remote_addr: target.remote_addr(),
        }
    }
}

fn get_origin(h: &hyper::HeaderMap) -> Option<std::net::IpAddr> {
    h.get("origin")
        .map(|v| v.to_str().ok())
        .flatten()
        .map(|v| url::Url::parse(v).ok())
        .flatten()
        .map(|v| v.host_str().map(|v| v.to_owned()))
        .flatten()
        .map(|v| v.parse::<std::net::IpAddr>().ok())
        .flatten()
}

fn get_referer(h: &hyper::HeaderMap) -> Option<std::net::IpAddr> {
    h.get("referer")
        .map(|v| v.to_str().ok())
        .flatten()
        .map(|v| url::Url::parse(v).ok())
        .flatten()
        .map(|v| v.host_str().map(|v| v.to_owned()))
        .flatten()
        .map(|v| v.parse::<std::net::IpAddr>().ok())
        .flatten()
}

fn get_x_forwarded_for(h: &hyper::HeaderMap) -> Option<std::net::IpAddr> {
    h.get("x-forwarded-for")
        .map(|v| v.to_str().ok())
        .flatten()
        .map(|v| v.split(",").map(|v| v.trim()).last())
        .flatten()
        .map(|v| v.parse::<std::net::IpAddr>().ok())
        .flatten()
}

// TODO refactor into extractor
pub fn ip_address(h: &hyper::HeaderMap) -> std::net::IpAddr {
    get_x_forwarded_for(h) // when behind reverse proxy
        .or_else(|| get_referer(h)) // when local not ws
        .or_else(|| get_origin(h)) // when local ws
        // fallback if buggy code above
        .unwrap_or_else(|| {
            tracing::error!("should have gotten IP by here");
            "127.0.0.1".parse().unwrap()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use hyper::StatusCode;

    pub struct TestApp {
        pub port: u16,
        pub address: String,

        app_handle: tokio::task::JoinHandle<ServerOutput>,
    }

    impl TestApp {
        async fn spawn() -> Self {
            let env_conf = conf::EnvConf::test_default();
            let env = conf::Env::Local;
            let conf = conf::Conf::new(env, env_conf);

            let app = Application::build(conf).await;
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
        let _app = TestApp::spawn().await;
    }

    #[tokio::test]
    async fn health() {
        let app = TestApp::spawn().await;

        let r = reqwest::get(format!("{}/api/health_check", app.address))
            .await
            .unwrap();

        assert_eq!(r.status(), StatusCode::OK);
    }
}
