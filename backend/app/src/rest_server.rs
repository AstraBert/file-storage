use std::sync::Arc;

use axum::{
    Json,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use tonic::transport::Server;

use crate::redis_utils::select_all_files;

mod redis_utils;
mod utils;

#[derive(Debug, Serialize, Deserialize)]
struct GetFilesResponse {
    files: Vec<String>,
}

struct AnyHowError(anyhow::Error);

// Tell axum how to convert `AnyHowError` into a response.
impl IntoResponse for AnyHowError {
    fn into_response(self) -> Response {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Something went wrong: {}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AnyHowError>`. That way you don't need to do that manually.
impl<E> From<E> for AnyHowError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

#[derive(Debug, Clone)]
struct AppState {
    redis_client: Arc<redis::Client>,
    grpc_client: Arc<
        proto::grpc::file_storage::file_storage_service_client::FileStorageServiceClient<Server>,
    >,
}

async fn get_files(State(state): State<AppState>) -> Result<Json<GetFilesResponse>, AnyHowError> {
    let files = select_all_files(&state.redis_client).await?;

    Ok(Json(GetFilesResponse { files }))
}

fn main() {
    println!("Hello, world!");
}
