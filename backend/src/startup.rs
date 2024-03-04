use crate::conf::Conf;
use static_routes::*;

use axum::{
    routing::{get, post},
    Router,
};

use std::sync::Arc;
use tower_http::{
    add_extension::AddExtensionLayer,
    compression::CompressionLayer,
    trace::{DefaultMakeSpan, DefaultOnRequest, DefaultOnResponse, TraceLayer},
    LatencyUnit, ServiceBuilderExt,
};

#[derive(Clone, Default)]
pub struct RequestIdProducer {
    counter: Arc<std::sync::atomic::AtomicU64>,
}

impl tower_http::request_id::MakeRequestId for RequestIdProducer {
    fn make_request_id<B>(
        &mut self,
        _request: &hyper::http::Request<B>,
    ) -> Option<tower_http::request_id::RequestId> {
        let request_id = self
            .counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst)
            .to_string()
            .parse()
            .unwrap();

        Some(tower_http::request_id::RequestId::new(request_id))
    }
}

pub fn router(_conf: &Conf) -> Router {
    use crate::routes::*;

    let routes = routes().api;

    let api_router = Router::new()
        .route(routes.health_check.get().postfix(), get(health_check))
        // TODO investigate why POST on /lobby gives 200
        .route("/snake/lobby", post(snake::create_lobby))
        .route("/snake/lobby/:name", get(snake::get_lobby))
        .route("/snake/ws", get(snake::ws::ws));

    let request_tracing_layer = tower::ServiceBuilder::new()
        .set_x_request_id(RequestIdProducer::default())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::DEBUG).include_headers(true))
                .make_span_with(|request: &hyper::http::Request<hyper::Body>| {
                    tracing::info_span!(
                        "request",
                        method = %request.method(),
                        uri = %request.uri(),
                        version = ?request.version(),
                        request_id = %request.headers().get("x-request-id").unwrap().to_str().unwrap(),
                    )
                })
                .on_request(DefaultOnRequest::new().level(tracing::Level::INFO))
                .on_response(
                    DefaultOnResponse::new()
                        .level(tracing::Level::INFO)
                        .latency_unit(LatencyUnit::Seconds),
                ),
        )
        .propagate_x_request_id();

    Router::new()
        .nest("/api", api_router)
        .layer(CompressionLayer::new())
        .layer(AddExtensionLayer::new(crate::mp_snake::Lobbies::default()))
        .layer(AddExtensionLayer::new(
            crate::mp_snake::PlayerUserNames::default(),
        ))
        .layer(request_tracing_layer)
}

pub struct Application {
    port: u16,
    server: std::pin::Pin<Box<dyn std::future::Future<Output = hyper::Result<()>> + Send>>,
    host: String,
}

impl Application {
    pub async fn build(conf: &Conf) -> Self {
        let address = format!("{}:{}", conf.env.host, conf.env.port);
        let listener = std::net::TcpListener::bind(&address).unwrap();
        tracing::info!("Listening on http://{}", address);
        let host = conf.env.host.clone();
        let port = listener.local_addr().unwrap().port();

        return Self {
            server: Box::pin(run(conf, listener)),
            port,
            host,
        };

        pub fn run(
            conf: &Conf,
            listener: std::net::TcpListener,
        ) -> impl std::future::Future<Output = hyper::Result<()>> {
            axum::Server::from_tcp(listener)
                .unwrap()
                .serve(router(conf).into_make_service_with_connect_info::<UserConnectInfo>())
        }
    }

    // needs to consume to produce 1 server max, and because I don't know better
    pub fn server(self) -> impl std::future::Future<Output = hyper::Result<()>> + Send {
        self.server
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn host(&self) -> &str {
        &self.host
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
