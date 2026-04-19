use std::{collections::HashMap, time::Duration};

use anyhow::anyhow;
use axum::http::HeaderMap;
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
pub struct RealmAccess {
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResourceAccess {
    pub roles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    // Standard JWT claims
    pub sub: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: String,
    pub aud: serde_json::Value, // Can be a string or array of strings
    pub jti: Option<String>,

    // Keycloak-specific
    pub typ: Option<String>,
    pub azp: Option<String>,
    pub sid: Option<String>,
    pub session_state: Option<String>,
    pub acr: Option<String>,
    pub scope: Option<String>,
    pub nonce: Option<String>,
    pub auth_time: Option<usize>,

    // Origins & access
    #[serde(rename = "allowed-origins")]
    pub allowed_origins: Option<Vec<String>>,
    pub realm_access: Option<RealmAccess>,
    pub resource_access: Option<HashMap<String, ResourceAccess>>,

    // User info
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub preferred_username: Option<String>,
    pub email: Option<String>,
    pub email_verified: Option<bool>,
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

pub fn extract_token(headers: &HeaderMap) -> anyhow::Result<String> {
    if !headers.contains_key("Authorization") {
        return Err(anyhow!("No Authorization header"));
    }
    let header_value = headers.get("Authorization").unwrap();
    let auth_header = header_value
        .to_str()
        .map_err(|_| anyhow!("Authorization error could not be converted to string"))?;
    if !auth_header.starts_with("Bearer ") {
        return Err(anyhow!("Authorization error does not start with 'Beaerer'"));
    }
    return Ok(auth_header.strip_prefix("Bearer ").unwrap().to_string());
}
