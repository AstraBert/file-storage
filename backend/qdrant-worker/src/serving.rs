use crate::{embeddings::embed_text, vectordb::VectorDB};
use anyhow::anyhow;
use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use axum::response::Response;
use axum::{Extension, middleware};
use axum::{Json, Router, extract::State, response::IntoResponse, routing::post};
use brakes::middleware::tower::TowerRateLimiterLayer;
use observability::init_tracing_subscriber;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::str::FromStr;
use std::sync::Arc;

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use tracing::{debug, error, info, instrument};
use utils::{
    AnyHowError, AuthConfig, Claims, DEFAULT_AUD, STATUS_COMPLETED, STATUS_FAILED, STATUS_STARTED,
    build_rate_limiter, extract_token, fetch_jwks,
};

const DEFAULT_PORT: u16 = 8000;
const DEFAULT_HOST: &str = "0.0.0.0";
const DEFAULT_RATE_LIMIT: u32 = 100;
const DEFAULT_SEARCH_LIMIT: u64 = 10;

pub struct RagServer {
    qdrant_url: String,
    pub collection_name: String,
    pub port: u16,
    pub host: IpAddr,
    pub rate_limit_per_second: u32,
}

#[derive(Deserialize, Serialize, Debug)]
struct RagRequest {
    query: String,
    limit: Option<u64>,
}

#[derive(Deserialize, Serialize, Debug)]
struct RagResponse {
    retrieved: Vec<String>,
}

#[derive(Clone, Debug)]
struct AppState {
    vectordb: VectorDB,
    auth_config: Arc<AuthConfig>,
}

#[derive(Deserialize, Serialize)]
struct RagError {
    status_code: usize,
    detail: String,
}

impl IntoResponse for RagError {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}

impl RagResponse {
    fn new(retrieved: Vec<String>) -> Self {
        Self { retrieved }
    }
}

impl RagServer {
    pub fn new(
        qdrant_url: String,
        collection_name: String,
        port: Option<u16>,
        host: Option<String>,
        rate_limit_per_second: Option<u32>,
    ) -> Self {
        let server_port = match port {
            Some(n) => n,
            None => DEFAULT_PORT,
        };
        let server_host = match host {
            Some(h) => {
                IpAddr::V4(Ipv4Addr::from_str(&h).expect("You should provide a valid IPv4 address"))
            }
            None => IpAddr::V4(
                Ipv4Addr::from_str(DEFAULT_HOST).expect("You should provide a valid IPv4 address"),
            ),
        };
        let server_rate_limit = match rate_limit_per_second {
            Some(r) => r,
            None => DEFAULT_RATE_LIMIT,
        };
        Self {
            qdrant_url,
            collection_name,
            host: server_host,
            port: server_port,
            rate_limit_per_second: server_rate_limit,
        }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        let _guard = init_tracing_subscriber();
        let cache = memcache::connect("memcache://memcached:11211")?;
        log::info!("Connected to memcached");
        let vectordb = VectorDB::new(self.qdrant_url.clone(), self.collection_name.clone())?;
        let coll_loaded = vectordb.check_collection_ready().await?;
        if coll_loaded == 0 {
            log::warn!("Vector database does not contain any vectors");
        }
        log::info!("Connected to Qdrant and checked collection existence");
        let auth_config = Arc::new(AuthConfig::new(
            "http://keycloak:8080".to_string(),
            "file-storage".to_string(),
        ));
        let state = AppState {
            vectordb,
            auth_config,
        };
        let post_rl = build_rate_limiter(&cache, self.rate_limit_per_second);
        let post_rl_layer =
            TowerRateLimiterLayer::default(post_rl, |r: &axum::http::Request<Body>| {
                r.headers()
                    .get("x-forwarded-for")
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string()
            });

        let app = Router::new()
            .route("/search", post(rag).layer(post_rl_layer))
            .layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            ))
            .with_state(state);
        let addr = SocketAddr::from((self.host, self.port));
        tracing::info!("listening on {}", addr);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        info!("Server listening on {}", addr.to_string());
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await?;

        Ok(())
    }
}

#[instrument(skip_all, name = "auth_middleware")]
async fn auth_middleware(
    State(state): State<AppState>,
    headers: HeaderMap,
    mut request: Request,
    next: Next,
) -> Result<Response, AnyHowError> {
    info!(event = "auth_middleware", status = STATUS_STARTED,);

    let token = extract_token(&headers).map_err(|e| {
        tracing::error!(
            event = "auth_middleware",
            error = e.to_string(),
            status = STATUS_FAILED,
        );
        AnyHowError(e, Some(StatusCode::UNAUTHORIZED))
    })?;

    let header = decode_header(&token).map_err(|e| {
        error!(
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
            error!(
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
        error!(
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
        error!(
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
        error!(
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
    info!(event = "auth_middleware", status = STATUS_COMPLETED,);

    request.extensions_mut().insert(token_data.claims);

    Ok(next.run(request).await)
}

#[instrument]
async fn rag(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(payload): Json<RagRequest>,
) -> Result<Json<RagResponse>, RagError> {
    let query_text = payload.query.clone();
    let embedding = embed_text(query_text);
    let search_limit = match payload.limit {
        Some(l) => l,
        None => DEFAULT_SEARCH_LIMIT,
    };
    info!(event="rag_search", status = STATUS_STARTED, data_id = %payload.query, "Starting vector search operation");
    let now = tokio::time::Instant::now();
    let results = match state
        .vectordb
        .search(
            embedding,
            search_limit,
            format!("{}-{}", claims.sub, claims.iss).as_str(),
        )
        .await
    {
        Ok(v) => v,
        Err(e) => {
            return Err(RagError {
                status_code: 500,
                detail: format!("Could not retrieve results because of {}", e),
            });
        }
    };
    let elapsed = now.elapsed().as_millis();
    debug!(event="rag_search", status = STATUS_COMPLETED, data_id = %payload.query, "Total retrieved results: {}/{}", results.len(), search_limit);
    info!(event="rag_search", status = STATUS_COMPLETED, data_id = %payload.query, "Ended vector search operation in {} ms", elapsed);
    Ok(Json(RagResponse::new(results)))
}
