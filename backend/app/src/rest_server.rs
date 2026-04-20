use std::{collections::HashSet, net::SocketAddr, sync::Arc};

use crate::{
    redis_utils::{
        FileMetadata, check_file_existence_and_copies, delete_file_metadata,
        get_all_files_and_metadata, hset_file_metadata,
    },
    utils::{AuthConfig, Claims, build_rate_limiter, extract_token, fetch_jwks},
};
use anyhow::anyhow;
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{Multipart, Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use brakes::middleware::tower::TowerRateLimiterLayer;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use proto::grpc::file_storage::{DeleteObjectRequest, GetPresignedUrlRequest, StoreFileRequest};
use serde::{Deserialize, Serialize};
use tonic::transport::Channel;

mod redis_utils;
mod utils;

const BUCKET_NAME: &str = "files";
const DEFAULT_AUD: &str = "account";

#[derive(Debug, Serialize, Deserialize)]
struct GetFilesResponse {
    files: Vec<FileMetadata>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PresignedUrlParams {
    expires_in: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct GetPresignedUrlResponse {
    presigned_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CheckFileExistenceResponse {
    file_name: String,
}

struct AnyHowError(anyhow::Error, Option<StatusCode>);

// Tell axum how to convert `AnyHowError` into a response.
impl IntoResponse for AnyHowError {
    fn into_response(self) -> Response {
        (
            self.1.unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
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
        Self(err.into(), None)
    }
}

#[derive(Debug, Clone)]
struct AppState {
    redis_client: Arc<redis::Client>,
    grpc_client: Arc<
        proto::grpc::file_storage::file_storage_service_client::FileStorageServiceClient<Channel>,
    >,
    auth_config: Arc<AuthConfig>,
}

async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AnyHowError> {
    let token =
        extract_token(&headers).map_err(|e| AnyHowError(e, Some(StatusCode::UNAUTHORIZED)))?;
    let header = decode_header(&token).map_err(|e| {
        AnyHowError(
            anyhow!("Impossible to decode header: {}", e.to_string()),
            Some(StatusCode::UNAUTHORIZED),
        )
    })?;

    let jwks = fetch_jwks(&state.auth_config.jwks_verification_url())
        .await
        .map_err(|e| {
            AnyHowError(
                anyhow!("Error while fetching JWKS: {}", e.to_string()),
                None,
            )
        })?;

    let kid = header.kid.ok_or(AnyHowError(
        anyhow!("No key ID associated with JWK"),
        Some(StatusCode::UNAUTHORIZED),
    ))?;
    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid.as_ref() == Some(&kid))
        .ok_or(AnyHowError(
            anyhow!("Could not find the JWK with the necessary key ID"),
            Some(StatusCode::UNAUTHORIZED),
        ))?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
        AnyHowError(
            anyhow!("Error while creating decoding the key: {}", e.to_string()),
            None,
        )
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    validation.aud = Some(HashSet::from([DEFAULT_AUD.to_string()]));

    let token_data = decode::<Claims>(&token, &decoding_key, &validation).map_err(|e| {
        AnyHowError(
            anyhow!("Error while decoding the key: {}", e.to_string()),
            Some(StatusCode::UNAUTHORIZED),
        )
    })?;

    log::debug!("Request successfully authenticated");

    request.extensions_mut().insert(token_data.claims);

    Ok(next.run(request).await)
}

async fn get_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<GetFilesResponse>, AnyHowError> {
    let files = get_all_files_and_metadata(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
    )
    .await
    .map_err(|e| {
        log::error!("{}", e.to_string());
        e
    })?;

    log::debug!("Successfully returned {:?} files (GET /files)", files.len());

    Ok(Json(GetFilesResponse { files }))
}

async fn check_file_existence(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(display_name): Path<String>,
) -> Result<Json<CheckFileExistenceResponse>, AnyHowError> {
    let file_name = check_file_existence_and_copies(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
        &display_name,
    )
    .await
    .map_err(|e| {
        log::error!("{}", e.to_string());
        e
    })?;

    log::debug!(
        "File existence successfully checked for file {} (GET /checks/<display_name>)",
        display_name
    );

    Ok(Json(CheckFileExistenceResponse { file_name }))
}

async fn post_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AnyHowError> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut file_name: Option<String> = None;
    let mut description: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AnyHowError(anyhow!(e.to_string()), None))?
    {
        let name = field.name().unwrap_or("").to_string();

        match name.as_str() {
            "file" => {
                file_data = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| {
                            AnyHowError(anyhow!(e.to_string()), Some(StatusCode::BAD_REQUEST))
                        })?
                        .to_vec(),
                );
            }
            "file_name" => {
                file_name = Some(field.text().await.map_err(|e| {
                    AnyHowError(anyhow!(e.to_string()), Some(StatusCode::BAD_REQUEST))
                })?)
            }
            "description" => {
                description = Some(field.text().await.map_err(|e| {
                    AnyHowError(anyhow!(e.to_string()), Some(StatusCode::BAD_REQUEST))
                })?);
            }
            _ => {
                // Skip unknown fields
            }
        }
    }

    let file_data = file_data.ok_or_else(|| {
        log::error!("No file data available");
        AnyHowError(
            anyhow!("No file data available"),
            Some(StatusCode::BAD_REQUEST),
        )
    })?;
    let file_name = file_name.unwrap_or_else(|| "unknown".to_string());
    let description = description.unwrap_or_default();
    log::debug!(
        "Found file data (length: {:?}) for file {}",
        file_data.len(),
        file_name
    );

    hset_file_metadata(
        &state.redis_client,
        &file_name,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
        file_data.len(),
        &description,
    )
    .await?;

    (*state.grpc_client)
        .clone()
        .store_file(StoreFileRequest {
            file_data,
            bucket_name: BUCKET_NAME.to_string(),
            key: format!("{}-{}-{}", claims.sub, claims.iss, file_name),
        })
        .await
        .map_err(|e| {
            log::error!("{}", e.to_string());
            AnyHowError(anyhow!(e.to_string()), None)
        })?;

    log::debug!("Successfully uploaded file to S3 (POST /uploads)");

    Ok((StatusCode::OK, "File uploaded successfully"))
}

async fn delete_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(display_name): Path<String>,
) -> Result<impl IntoResponse, AnyHowError> {
    delete_file_metadata(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
        &display_name,
    )
    .await?;
    log::debug!(
        "Successfully deleted file from Redis for file {}",
        &display_name
    );
    (*state.grpc_client)
        .clone()
        .delete_object(DeleteObjectRequest {
            bucket_name: BUCKET_NAME.to_string(),
            key: format!("{}-{}-{}", claims.sub, claims.iss, &display_name),
        })
        .await
        .map_err(|e| {
            log::error!("{}", e.to_string());
            AnyHowError(anyhow!(e.to_string()), None)
        })?;
    log::debug!(
        "Successfully deleted file from S3 for file {} (DELETE /files/<display_name>)",
        &display_name
    );

    Ok((StatusCode::NO_CONTENT, ""))
}

async fn get_file_presigned_url(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(display_name): Path<String>,
    Query(params): Query<PresignedUrlParams>,
) -> Result<Json<GetPresignedUrlResponse>, AnyHowError> {
    let response = (*state.grpc_client)
        .clone()
        .get_presigned_url(GetPresignedUrlRequest {
            bucket_name: BUCKET_NAME.to_string(),
            key: format!("{}-{}-{}", claims.sub, claims.iss, &display_name),
            expires_in: params.expires_in,
        })
        .await
        .map_err(|e| {
            log::error!("{}", e.to_string());
            AnyHowError(anyhow!(e.to_string()), None)
        })?;
    log::debug!(
        "Successfully obtained presigned URL for {} (GET /urls/<display_name>)",
        &display_name
    );
    Ok(Json(GetPresignedUrlResponse {
        presigned_url: response.into_inner().presigned_url,
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    let cache = memcache::connect("memcache://memcached:11211")?;
    log::info!("Connected to memcached");
    let redis_client = redis::Client::open("redis://redis:6379")?;
    log::info!("Connected to redis");
    let grpc_client =
        proto::grpc::file_storage::file_storage_service_client::FileStorageServiceClient::connect(
            "http://grpc-server:50051",
        )
        .await?;
    log::info!("Connected to grpc server");
    let auth_config = Arc::new(AuthConfig::new(
        "http://keycloak:8080".to_string(),
        "file-storage".to_string(),
    ));
    let app_state = AppState {
        redis_client: Arc::new(redis_client),
        grpc_client: Arc::new(grpc_client),
        auth_config: auth_config,
    };
    let post_rl = build_rate_limiter(&cache, 100);
    let delete_rl = build_rate_limiter(&cache, 100);
    let get_url_rl = build_rate_limiter(&cache, 1000);
    let check_exists_rl = build_rate_limiter(&cache, 100);
    let post_rl_layer = TowerRateLimiterLayer::default(post_rl, |r: &axum::http::Request<Body>| {
        r.headers()
            .get("x-forwarded-for")
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    });
    let delete_rl_layer =
        TowerRateLimiterLayer::default(delete_rl, |r: &axum::http::Request<Body>| {
            r.headers()
                .get("x-forwarded-for")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        });

    let check_exists_rl_layer =
        TowerRateLimiterLayer::default(check_exists_rl, |r: &axum::http::Request<Body>| {
            r.headers()
                .get("x-forwarded-for")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        });
    let get_url_rl_layer =
        TowerRateLimiterLayer::default(get_url_rl, |r: &axum::http::Request<Body>| {
            r.headers()
                .get("x-forwarded-for")
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        });
    let app = Router::new()
        .route("/files", get(get_files))
        .route(
            "/files/{display_name}",
            delete(delete_file).layer(delete_rl_layer),
        )
        .route("/uploads", post(post_file).layer(post_rl_layer))
        .route(
            "/checks/{display_name}",
            get(check_file_existence).layer(check_exists_rl_layer),
        )
        .route(
            "/urls/{display_name}",
            get(get_file_presigned_url).layer(get_url_rl_layer),
        )
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth_middleware,
        ))
        .with_state(app_state);
    let listener = tokio::net::TcpListener::bind("0.0.0.0:4444").await?;
    log::info!("Starting to serve the rest API on port 4444");
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}
