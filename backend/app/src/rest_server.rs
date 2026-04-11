use std::sync::Arc;

use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use proto::grpc::file_storage::StoreFileRequest;
use serde::{Deserialize, Serialize};
use tonic::transport::{Channel, Server};

use crate::redis_utils::{FileMetadata, get_all_files_and_metadata, hset_file_metadata};

mod redis_utils;
mod utils;

const BUCKET_NAME: &str = "files";

#[derive(Debug, Serialize, Deserialize)]
struct GetFilesResponse {
    files: Vec<FileMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PostFileRequest {
    display_name: String,
    full_path: String,
    base64_data: String,
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
        proto::grpc::file_storage::file_storage_service_client::FileStorageServiceClient<Channel>,
    >,
}

async fn get_files(State(state): State<AppState>) -> Result<Json<GetFilesResponse>, AnyHowError> {
    let files = get_all_files_and_metadata(&state.redis_client).await?;

    Ok(Json(GetFilesResponse { files }))
}

async fn post_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, StatusCode> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut description: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_name = field.file_name().map(|s| s.to_string());
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|_| StatusCode::BAD_REQUEST)?
                        .to_vec(),
                );
            }
            "description" => {
                description = Some(field.text().await.map_err(|_| StatusCode::BAD_REQUEST)?);
            }
            _ => {
                // Skip unknown fields
            }
        }
    }

    let file_data = file_data.ok_or(StatusCode::BAD_REQUEST)?;
    let file_name = file_name.unwrap_or_else(|| "unknown".to_string());
    let description = description.unwrap_or_default();

    hset_file_metadata(
        &state.redis_client,
        &file_name,
        file_data.len(),
        &description,
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    (*state.grpc_client)
        .clone()
        .store_file(StoreFileRequest {
            file_data,
            bucket_name: BUCKET_NAME.to_string(),
            key: file_name,
        })
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((StatusCode::OK, "File uploaded successfully"))
}

fn main() {
    println!("Hello, world!");
}
