use tokio::fs;
use std::path::Path;

pub async fn read_file(path: impl AsRef<Path>) -> String {
    match fs::read_to_string(path).await {
        Ok(content) => content,
        Err(e) => format!("Error reading file: {}", e),
    }
}
