use std::time::Duration;

use brakes::{RateLimiter, backend::memcache::MemCache, types::token_bucket::TokenBucket};
use memcache::Client;

pub fn build_rate_limiter(cache: &Client) -> RateLimiter<TokenBucket, MemCache> {
    RateLimiter::builder()
        .with_backend(MemCache::new(cache.clone()))
        .with_limiter(TokenBucket::new(100, Duration::from_secs(1)))
        .build()
}
