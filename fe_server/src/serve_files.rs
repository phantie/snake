#[derive(Debug)]
#[allow(unused)]
pub struct ServedFile<'a> {
    pub path: &'a str,
    pub size: &'a str,
}

pub fn file_response(
    contents: impl Into<axum::body::Full<bytes::Bytes>>,
    path: impl AsRef<std::path::Path>,
    modified: std::time::SystemTime,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    let last_modified = httpdate::fmt_http_date(modified);
    let mime_type = mime_guess::from_path(path).first_or_text_plain();
    axum::http::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_str(mime_type.as_ref()).unwrap(),
        )
        .header(axum::http::header::LAST_MODIFIED, last_modified)
        .body(axum::body::boxed(contents.into()))
        .unwrap_or_else(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response())
}
