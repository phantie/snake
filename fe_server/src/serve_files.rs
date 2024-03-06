#[derive(Debug)]
#[allow(unused)]
pub struct ServedFile<'a> {
    pub path: &'a str,
    pub size: &'a str,
}

pub fn file_response(file: &File) -> axum::response::Response {
    use axum::response::IntoResponse;
    let last_modified = httpdate::fmt_http_date(file.modified);
    let mime_type = mime_guess::from_path(&file.path.as_ref()).first_or_text_plain();
    // tracing::warn!("mime type {mime_type} derived from {:?}", &file.path);

    axum::http::Response::builder()
        .status(axum::http::StatusCode::OK)
        .header(
            axum::http::header::CONTENT_TYPE,
            axum::http::HeaderValue::from_str(mime_type.as_ref()).unwrap(),
        )
        .header(axum::http::header::LAST_MODIFIED, last_modified)
        .body(axum::body::boxed(axum::body::Full::<bytes::Bytes>::from(
            file.contents.clone(),
        )))
        .unwrap_or_else(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug)]
pub struct File {
    pub contents: Vec<u8>,
    pub request_path: String,
    pub path: Box<std::path::PathBuf>,
    pub modified: std::time::SystemTime,
}

#[derive(Default, Debug, Clone, derived_deref::Deref)]
pub struct Cache {
    #[target]
    request_path_to_file: Arc<RwLock<HashMap<String, Arc<File>>>>,
    disk_path_to_file: Arc<RwLock<HashMap<Box<std::path::PathBuf>, Arc<File>>>>,
}

impl Cache {
    pub async fn get_request_path(&self, path: &str) -> Option<Arc<File>> {
        self.request_path_to_file
            .read()
            .await
            .get(path)
            .map(Clone::clone)
    }

    pub async fn get_disk_path(&self, path: &std::path::PathBuf) -> Option<Arc<File>> {
        self.disk_path_to_file
            .read()
            .await
            .get(path)
            .map(Clone::clone)
    }

    pub async fn insert(&self, path: String, file: Arc<File>) {
        self.request_path_to_file
            .write()
            .await
            .insert(path, file.clone());
        self.disk_path_to_file
            .write()
            .await
            .insert(file.path.clone(), file);
    }
}
