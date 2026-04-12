use std::time::Duration;

use axum::http::{HeaderMap, StatusCode};
use brakes::{RateLimiter, backend::memcache::MemCache, types::token_bucket::TokenBucket};
use memcache::Client;
use serde::{Deserialize, Serialize};

pub fn build_rate_limiter(cache: &Client, rps: u32) -> RateLimiter<TokenBucket, MemCache> {
    RateLimiter::builder()
        .with_backend(MemCache::new(cache.clone()))
        .with_limiter(TokenBucket::new(rps, Duration::from_secs(1)))
        .build()
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: String,
    pub exp: usize,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    // Add other claims you need
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    keycloak_base_url: String,
    realm: String,
}

impl AuthConfig {
    pub fn new(keycloak_base_url: String, realm: String) -> Self {
        Self {
            keycloak_base_url,
            realm,
        }
    }

    pub fn jwks_verification_url(&self) -> String {
        format!(
            "{}/realms/{}/protocol/openid-connect/certs",
            self.keycloak_base_url, self.realm
        )
    }
}

#[derive(Debug, Deserialize)]
pub struct Jwks {
    pub keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize)]
pub struct Jwk {
    pub kid: Option<String>,
    pub n: String,
    pub e: String,
}

pub async fn fetch_jwks(url: &str) -> Result<Jwks, reqwest::Error> {
    reqwest::get(url).await?.json().await
}

pub fn extract_token(headers: &HeaderMap) -> Result<String, StatusCode> {
    if !headers.contains_key("Authorization") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let header_value = headers.get("Authorization").unwrap();
    let auth_header = header_value
        .to_str()
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    return Ok(auth_header.strip_prefix("Bearer ").unwrap().to_string());
}
