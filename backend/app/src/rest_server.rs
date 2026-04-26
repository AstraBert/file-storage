use std::{collections::HashSet, fmt, net::SocketAddr, sync::Arc};

use crate::redis_utils::{
    FileMetadata, check_file_existence_and_copies, delete_file_metadata,
    get_all_files_and_metadata, get_file_description, hset_file_metadata,
};
use anyhow::anyhow;
use axum::{
    Extension, Json, Router,
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
};
use brakes::middleware::tower::TowerRateLimiterLayer;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use observability::init_tracing_subscriber;
use proto::grpc::file_storage::{DeleteObjectRequest, GetPresignedUrlRequest, StoreFileRequest};
use rabbitmq_stream_client::{
    Environment, NoDedup, Producer,
    error::StreamCreateError,
    types::{ByteCapacity, Message, ResponseCode},
};
use serde::{Deserialize, Serialize};
use tonic::transport::Channel;
use utils::{
    AnyHowError, AuthConfig, Claims, DEFAULT_AUD, MessageAction, MessageData, STATUS_COMPLETED,
    STATUS_FAILED, STATUS_STARTED, build_rate_limiter, extract_token, fetch_jwks,
};

mod redis_utils;

const BUCKET_NAME: &str = "files";
const STREAM_NAME: &str = "worker_queue";

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

#[derive(Clone)]
pub struct DebuggableProducer(pub Arc<Producer<NoDedup>>);

impl fmt::Debug for DebuggableProducer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Producer<NoDedup>")
    }
}

#[derive(Debug, Clone)]
struct AppState {
    redis_client: Arc<redis::Client>,
    grpc_client: Arc<
        proto::grpc::file_storage::file_storage_service_client::FileStorageServiceClient<Channel>,
    >,
    auth_config: Arc<AuthConfig>,
    rabbitmq_producer: DebuggableProducer,
}

#[tracing::instrument(skip_all, name = "auth_middleware")]
async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AnyHowError> {
    tracing::info!(event = "auth_middleware", status = STATUS_STARTED,);

    let token = extract_token(&headers).map_err(|e| {
        tracing::error!(
            event = "auth_middleware",
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        AnyHowError(e, Some(StatusCode::UNAUTHORIZED))
    })?;

    let header = decode_header(&token).map_err(|e| {
        tracing::error!(
            event = "auth_middleware",
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        AnyHowError(
            anyhow!("Impossible to decode header: {}", e.to_string()),
            Some(StatusCode::UNAUTHORIZED),
        )
    })?;

    let jwks = fetch_jwks(&state.auth_config.jwks_verification_url())
        .await
        .map_err(|e| {
            tracing::error!(
                event = "auth_middleware",
                error = e.to_string(),
                status = STATUS_FAILED,
            );
            AnyHowError(
                anyhow!("Error while fetching JWKS: {}", e.to_string()),
                None,
            )
        })?;

    let kid = header.kid.ok_or_else(|| {
        tracing::error!(
            event = "auth_middleware",
            error = "No key ID associated with JWK",
            status = STATUS_FAILED,
        );
        AnyHowError(
            anyhow!("No key ID associated with JWK"),
            Some(StatusCode::UNAUTHORIZED),
        )
    })?;

    let jwk = jwks
        .keys
        .iter()
        .find(|k| k.kid.as_ref() == Some(&kid))
        .ok_or_else(|| {
            tracing::error!(
                event = "auth_middleware",
                error = "Could not find the JWK with the necessary key ID",
                status = STATUS_FAILED,
            );
            AnyHowError(
                anyhow!("Could not find the JWK with the necessary key ID"),
                Some(StatusCode::UNAUTHORIZED),
            )
        })?;

    let decoding_key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e).map_err(|e| {
        tracing::error!(
            event = "auth_middleware",
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        AnyHowError(
            anyhow!("Error while creating decoding the key: {}", e.to_string()),
            None,
        )
    })?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;
    validation.aud = Some(HashSet::from([DEFAULT_AUD.to_string()]));

    let token_data = decode::<Claims>(&token, &decoding_key, &validation).map_err(|e| {
        tracing::error!(
            event = "auth_middleware",
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        AnyHowError(
            anyhow!("Error while decoding the key: {}", e.to_string()),
            Some(StatusCode::UNAUTHORIZED),
        )
    })?;

    log::debug!("Request successfully authenticated");
    tracing::info!(event = "auth_middleware", status = STATUS_COMPLETED,);

    request.extensions_mut().insert(token_data.claims);

    Ok(next.run(request).await)
}

#[tracing::instrument]
async fn get_files(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
) -> Result<Json<GetFilesResponse>, AnyHowError> {
    tracing::info!(event = "get_files_rest", status = STATUS_STARTED,);
    let files = get_all_files_and_metadata(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
    )
    .await
    .map_err(|e| {
        log::error!("{}", e.to_string());
        tracing::error!(
            event = "get_files_rest",
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        e
    })?;

    tracing::info!(
        event = "get_files_rest",
        count = files.len(),
        status = STATUS_COMPLETED,
    );
    log::debug!("Successfully returned {:?} files (GET /files)", files.len());

    Ok(Json(GetFilesResponse { files }))
}

#[tracing::instrument]
async fn check_file_existence(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(display_name): Path<String>,
) -> Result<Json<CheckFileExistenceResponse>, AnyHowError> {
    tracing::info!(
        event = "check_file_existence_rest",
        file = display_name,
        status = STATUS_STARTED,
    );
    let file_name = check_file_existence_and_copies(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
        &display_name,
    )
    .await
    .map_err(|e| {
        log::error!("{}", e.to_string());
        tracing::error!(
            event = "check_file_existence_rest",
            file = display_name,
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        e
    })?;

    tracing::info!(
        event = "check_file_existence_rest",
        file = display_name,
        status = STATUS_COMPLETED,
    );
    log::debug!(
        "File existence successfully checked for file {} (GET /checks/<display_name>)",
        display_name
    );

    Ok(Json(CheckFileExistenceResponse { file_name }))
}

#[tracing::instrument]
async fn post_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    mut multipart: Multipart,
) -> Result<impl IntoResponse, AnyHowError> {
    tracing::info!(event = "post_file_rest", status = STATUS_STARTED,);
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
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| {
        log::error!("No file data available");
        tracing::error!(
            event = "post_file_rest",
            error = "No file data available",
            status = STATUS_FAILED,
        );
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
    .await
    .map_err(|e| {
        tracing::error!(
            event = "post_file_rest",
            file = file_name,
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        e
    })?;

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
            tracing::error!(
                event = "post_file_rest",
                file = file_name,
                error = e.to_string(),
                status = STATUS_FAILED,
            );
            AnyHowError(anyhow!(e.to_string()), None)
        })?;

    let data = MessageData {
        content: format!("{}\n\n{}", &file_name, &description),
        user_identity: format!("{}-{}", claims.sub, claims.iss),
        action: MessageAction::Create,
    };
    let message = serde_json::to_string(&data)?;
    let msg = Message::builder().body(message).build();
    state.rabbitmq_producer.0.send(msg, |_| async {}).await?;
    log::debug!("Sent data to RabbitMQ");

    tracing::info!(
        event = "post_file_rest",
        file = file_name,
        status = STATUS_COMPLETED,
    );
    log::debug!("Successfully uploaded file to S3 (POST /uploads)");

    Ok((StatusCode::OK, "File uploaded successfully"))
}

#[tracing::instrument]
async fn delete_file(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(display_name): Path<String>,
) -> Result<impl IntoResponse, AnyHowError> {
    tracing::info!(
        event = "delete_file_rest",
        file = display_name,
        status = STATUS_STARTED,
    );
    let description = get_file_description(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
        &display_name,
    )
    .await
    .map_err(|e| {
        tracing::error!(
            event = "delete_file_rest",
            file = display_name,
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        e
    })?;
    log::debug!("Obtained file description from Redis before deletion");
    delete_file_metadata(
        &state.redis_client,
        format!("{}-{}", claims.sub, claims.iss).as_str(),
        &display_name,
    )
    .await
    .map_err(|e| {
        tracing::error!(
            event = "delete_file_rest",
            file = display_name,
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        e
    })?;

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
            tracing::error!(
                event = "delete_file_rest",
                file = display_name,
                error = e.to_string(),
                status = STATUS_FAILED,
            );
            AnyHowError(anyhow!(e.to_string()), None)
        })?;
    log::debug!(
        "Successfully deleted file from S3 for file {} (DELETE /files/<display_name>)",
        &display_name
    );

    let data = MessageData {
        action: MessageAction::Delete,
        user_identity: format!("{}-{}", claims.sub, claims.iss),
        content: format!("{}\n\n{}", &display_name, &description),
    };
    let message = serde_json::to_string(&data)?;
    let msg = Message::builder().body(message).build();
    state.rabbitmq_producer.0.send(msg, |_| async {}).await?;

    log::debug!(
        "Successfully sent message to RabbitMQ to delete record {} from Qdrant (DELETE /files/<display_name>)",
        &display_name
    );

    tracing::info!(
        event = "delete_file_rest",
        file = display_name,
        status = STATUS_COMPLETED,
    );

    Ok((StatusCode::NO_CONTENT, ""))
}

#[tracing::instrument]
async fn get_file_presigned_url(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(display_name): Path<String>,
    Query(params): Query<PresignedUrlParams>,
) -> Result<Json<GetPresignedUrlResponse>, AnyHowError> {
    tracing::info!(
        event = "get_file_presigned_url_rest",
        file = display_name,
        expiration = params.expires_in,
        status = STATUS_STARTED,
    );
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
            tracing::error!(
                event = "get_file_presigned_url_rest",
                file = display_name,
                expiration = params.expires_in,
                error = e.to_string(),
                status = STATUS_FAILED,
            );
            AnyHowError(anyhow!(e.to_string()), None)
        })?;
    log::debug!(
        "Successfully obtained presigned URL for {} (GET /urls/<display_name>)",
        &display_name
    );
    tracing::info!(
        event = "get_file_presigned_url_rest",
        file = display_name,
        expiration = params.expires_in,
        status = STATUS_COMPLETED,
    );
    Ok(Json(GetPresignedUrlResponse {
        presigned_url: response.into_inner().presigned_url,
    }))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _guard = init_tracing_subscriber();
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
    let environment = Environment::builder()
        .host("rabbitmq")
        .port(5552)
        .username(
            &std::env::var("RABBITMQ_DEFAULT_USER")
                .expect("Should have RABBITMQ_DEFAULT_USER set in env"),
        )
        .password(
            &std::env::var("RABBITMQ_DEFAULT_PASS")
                .expect("Should have RABBITMQ_DEFAULT_PASS set in env"),
        )
        .build()
        .await?;

    // create the rabbitmq stream if it does not already exist
    let create_response = environment
        .stream_creator()
        .max_length(ByteCapacity::GB(5))
        .create(STREAM_NAME)
        .await;

    if let Err(StreamCreateError::Create { stream: _, status }) = create_response {
        match status {
            // we can ignore this error because the stream already exists
            ResponseCode::StreamAlreadyExists => {
                log::info!("Stream already exists")
            }
            err => {
                log::error!("Error creating stream: {:?} {:?}", STREAM_NAME, err);
            }
        }
    }
    let producer = environment.producer().build(STREAM_NAME).await.unwrap();
    log::info!("Connected to RabbitMQ stream");
    let auth_config = Arc::new(AuthConfig::new(
        "http://keycloak:8080".to_string(),
        "file-storage".to_string(),
    ));
    let app_state = AppState {
        redis_client: Arc::new(redis_client),
        grpc_client: Arc::new(grpc_client),
        auth_config: auth_config,
        rabbitmq_producer: DebuggableProducer(Arc::new(producer)),
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
        .layer(DefaultBodyLimit::disable())
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
