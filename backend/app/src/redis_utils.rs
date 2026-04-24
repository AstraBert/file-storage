extern crate redis;

use anyhow::{Result, anyhow};
use redis::AsyncTypedCommands;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, path};
use tokio_stream::StreamExt;

#[derive(Debug, Serialize, Deserialize)]
pub struct FileMetadata {
    pub display_name: String,
    pub file_size: usize,
    pub file_description: String,
}

impl FileMetadata {
    pub fn from_hset(hset: HashMap<String, String>) -> Result<Self> {
        if hset.len() != 3 {
            return Err(anyhow!("Unexpected length: {:?}", hset.len()));
        }

        let mut file_size: usize = 0;
        let mut display_name: String = String::from("unknown");
        let mut file_description: String = String::from("no description");

        for (k, v) in hset {
            match k.as_str() {
                "size" => file_size = v.parse::<usize>()?,
                "display_name" => display_name = v,
                "file_description" => file_description = v,
                _ => continue,
            }
        }

        Ok(Self {
            display_name,
            file_size,
            file_description,
        })
    }
}

pub async fn hset_file_metadata(
    client: &redis::Client,
    display_name: &str,
    user_identifier: &str,
    file_size: usize,
    file_description: &str,
) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;

    conn.hset_multiple(
        format!("{}-file:{}", user_identifier, display_name),
        &[
            ("display_name", display_name),
            ("size", &file_size.to_string()),
            ("file_description", file_description),
        ],
    )
    .await?;
    Ok(())
}

pub async fn check_file_existence_and_copies(
    client: &redis::Client,
    user_identifier: &str,
    display_name: &str,
) -> Result<String> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let result = conn
        .exists(format!("{}-file:{}", user_identifier, display_name))
        .await?;
    if result {
        let p = path::PathBuf::from(display_name);
        let stem = p.file_stem().unwrap();
        let ext = p.extension().unwrap_or_default();
        let keys: Vec<String> = conn
            .scan_match(format!("file:{}_*", stem.to_string_lossy().to_string()))
            .await?
            .map(|r| r.unwrap())
            .collect()
            .await;
        if keys.len() > 0 {
            let new_display_name = format!(
                "{}_{:?}.{}",
                stem.to_string_lossy().to_string(),
                keys.len(),
                ext.to_string_lossy().to_string(),
            );
            return Ok(new_display_name.trim_end_matches(".").to_string());
        }
    }
    Ok(display_name.to_string())
}

pub async fn get_all_files_and_metadata(
    client: &redis::Client,
    user_identifier: &str,
) -> Result<Vec<FileMetadata>> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let mut files: Vec<FileMetadata> = Vec::new();

    let keys: Vec<String> = conn
        .scan_match(format!("{}-file:*", user_identifier))
        .await?
        .map(|r| r.unwrap())
        .collect()
        .await;

    for key in keys {
        let hset = conn.hgetall(&key).await?;
        let file = FileMetadata::from_hset(hset)?;
        files.push(file);
    }

    Ok(files)
}

pub async fn delete_file_metadata(
    client: &redis::Client,
    user_identifier: &str,
    display_name: &str,
) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    conn.del(format!("{}-file:{}", user_identifier, display_name))
        .await?;
    Ok(())
}

pub async fn check_index_exists(client: &redis::Client, user_identifier: &str) -> Result<bool> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let result: redis::RedisResult<redis::Value> = redis::cmd("FT.info")
        .arg(user_identifier)
        .query_async(&mut conn)
        .await;
    match result {
        Ok(_) => Ok(true),
        Err(_) => Ok(false),
    }
}

pub async fn create_search_index(client: &redis::Client, user_identifier: &str) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let _: redis::Value = redis::cmd("FT.CREATE")
        .arg(user_identifier)
        .arg("ON")
        .arg("HASH")
        .arg("PREFIX")
        .arg(1)
        .arg(format!("{}-file:", user_identifier))
        .arg("SCHEMA")
        .arg("display_name")
        .arg("TEXT")
        .arg("file_description")
        .arg("TEXT")
        .query_async(&mut conn)
        .await?;
    Ok(())
}

pub async fn full_text_search_description(
    client: &redis::Client,
    user_identifier: &str,
    search_term: &str,
) -> Result<()> {
    let exists = check_index_exists(client, user_identifier).await?;
    if !exists {
        create_search_index(client, user_identifier).await?;
    }
    let mut conn = client.get_multiplexed_async_connection().await?;
    let _search_result: redis::Value = redis::cmd("FT.SEARCH")
        .arg(user_identifier)
        .arg(format!("@file_description:\"{}\"", search_term))
        .query_async(&mut conn)
        .await?;
    Ok(())
}

pub async fn full_text_search_title(
    client: &redis::Client,
    user_identifier: &str,
    search_term: &str,
) -> Result<()> {
    let exists = check_index_exists(client, user_identifier).await?;
    if !exists {
        create_search_index(client, user_identifier).await?;
    }
    let mut conn = client.get_multiplexed_async_connection().await?;
    let _search_result: redis::Value = redis::cmd("FT.SEARCH")
        .arg(user_identifier)
        .arg(format!("@display_name:\"{}\"", search_term))
        .query_async(&mut conn)
        .await?;
    Ok(())
}
