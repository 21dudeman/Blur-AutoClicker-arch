use std::time::Duration;

use futures_util::StreamExt;
use reqwest::header::USER_AGENT;
use serde::Deserialize;
use tauri::AppHandle;

use crate::error::AppError;
use crate::error::AppResult;

const MAX_RESPONSE_BYTES: u64 = 1_000_000;

fn ensure_size(len: Option<u64>) -> Result<(), AppError> {
    if let Some(n) = len {
        if n > MAX_RESPONSE_BYTES {
            return Err(AppError::Network("Response too large".into()));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
}

#[tauri::command]
pub async fn fetch_changelog() -> AppResult<String> {
    let url = "https://raw.githubusercontent.com/21dudeman/Blur-AutoClicker-arch/main/CHANGELOG.md";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(format!("Failed to build HTTP client: {}", e)))?;

    let response = client
        .get(url)
        .header(USER_AGENT, "BlurAutoClicker")
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Network error: {}", e)))?
        .error_for_status()
        .map_err(|e| AppError::Network(format!("HTTP error: {}", e)))?;

    ensure_size(response.content_length())?;

    let mut stream = response.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk =
            chunk.map_err(|e| AppError::Network(format!("Failed to read changelog: {}", e)))?;
        if (buf.len() + chunk.len()) as u64 > MAX_RESPONSE_BYTES {
            return Err(AppError::Network(format!(
                "Changelog too large (>{} bytes)",
                MAX_RESPONSE_BYTES
            )));
        }
        buf.extend_from_slice(&chunk);
    }
    let text = String::from_utf8(buf)
        .map_err(|e| AppError::Network(format!("Changelog not valid UTF-8: {}", e)))?;

    Ok(text.trim().to_string())
}

#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> AppResult<Option<CheckUpdateResult>> {
    let current_version = app.config().version.clone().unwrap_or("0.0.0".into());
    let url = "https://api.github.com/repos/21dudeman/Blur-AutoClicker-arch/releases/latest";
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| AppError::Network(format!("Failed to build HTTP client: {}", e)))?;

    let response = client
        .get(url)
        .header(USER_AGENT, "BlurAutoClicker")
        .send()
        .await
        .map_err(|e| AppError::Network(format!("Network error: {}", e)))?
        .error_for_status()
        .map_err(|e| AppError::Network(format!("HTTP error: {}", e)))?;

    ensure_size(response.content_length())?;

    if response.status().is_success() {
        let mut stream = response.bytes_stream();
        let mut buf = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|e| AppError::Network(format!("Failed to read release: {}", e)))?;
            if (buf.len() + chunk.len()) as u64 > MAX_RESPONSE_BYTES {
                return Err(AppError::Network(format!(
                    "Release response too large (>{} bytes)",
                    MAX_RESPONSE_BYTES
                )));
            }
            buf.extend_from_slice(&chunk);
        }
        let text = String::from_utf8(buf)
            .map_err(|e| AppError::Network(format!("Release not valid UTF-8: {}", e)))?;

        let release: GithubRelease = serde_json::from_str(text.trim())
            .map_err(|e| AppError::Network(format!("Failed to parse release: {}", e)))?;

        if is_update_available(&release.tag_name, &current_version) {
            return Ok(Some(CheckUpdateResult {
                current_version: current_version.clone(),
                latest_version: release.tag_name,
                update_available: true,
            }));
        }
    }

    Ok(Some(CheckUpdateResult {
        current_version,
        latest_version: String::new(),
        update_available: false,
    }))
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckUpdateResult {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
}

fn is_update_available(remote: &str, local: &str) -> bool {
    let r_ver = remote.trim_start_matches('v');
    let l_ver = local.trim_start_matches('v');

    let r_parts: Vec<&str> = r_ver.split('.').collect();
    let l_parts: Vec<&str> = l_ver.split('.').collect();

    let max_len = std::cmp::max(r_parts.len(), l_parts.len());

    for i in 0..max_len {
        let r_num = r_parts
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);
        let l_num = l_parts
            .get(i)
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(0);

        if r_num > l_num {
            return true;
        }
        if r_num < l_num {
            return false;
        }
    }
    false
}
