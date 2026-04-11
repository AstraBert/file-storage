extern crate redis;

use anyhow::{Result, anyhow};
use redis::AsyncTypedCommands;

pub async fn set_file_metadata(
    client: &redis::Client,
    file_path: &str,
    display_name: &str,
) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    conn.set(format!("file:{}", display_name), file_path)
        .await?;
    Ok(())
}

pub async fn get_file_metadata(client: &redis::Client, display_name: &str) -> Result<String> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let result = conn.get(format!("file:{}", display_name)).await?;
    match result {
        Some(s) => Ok(s),
        None => Err(anyhow!(
            "Could not find file path for display name '{}'",
            display_name
        )),
    }
}

pub async fn delete_file_metadata(client: &redis::Client, display_name: &str) -> Result<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    conn.del(format!("file:{}", display_name)).await?;
    Ok(())
}

pub async fn select_all_files(client: &redis::Client) -> Result<Vec<String>> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let mut results: redis::AsyncIter<String> = conn.scan_match("file:*").await?;
    let mut files: Vec<String> = Vec::new();
    while let Some(r) = results.next_item().await {
        let res = r?;
        files.push(res);
    }
    Ok(files)
}
