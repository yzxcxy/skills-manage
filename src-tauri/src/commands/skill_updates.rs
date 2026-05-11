use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tauri::State;

use crate::AppState;

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillUpdateInfo {
    pub skill_id: String,
    pub skill_name: String,
    pub has_update: bool,
    pub remote_url: String,
    pub error: Option<String>,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

async fn download_remote_text(client: &reqwest::Client, url: &str) -> Result<String, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Download failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("Download returned {}", resp.status()));
    }

    resp.text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))
}

async fn check_single_skill_update(
    client: &reqwest::Client,
    skill_id: &str,
    skill_name: &str,
    file_path: &str,
    remote_url: &str,
) -> SkillUpdateInfo {
    let local_content = match std::fs::read_to_string(file_path) {
        Ok(c) => c,
        Err(e) => {
            return SkillUpdateInfo {
                skill_id: skill_id.to_string(),
                skill_name: skill_name.to_string(),
                has_update: false,
                remote_url: remote_url.to_string(),
                error: Some(format!("Failed to read local file: {}", e)),
            };
        }
    };

    let remote_content = match download_remote_text(client, remote_url).await {
        Ok(c) => c,
        Err(e) => {
            return SkillUpdateInfo {
                skill_id: skill_id.to_string(),
                skill_name: skill_name.to_string(),
                has_update: false,
                remote_url: remote_url.to_string(),
                error: Some(e),
            };
        }
    };

    // Normalize line endings to ignore CRLF/LF differences
    let normalize = |s: &str| s.replace("\r\n", "\n").trim().to_string();
    let has_update = normalize(&local_content) != normalize(&remote_content);

    SkillUpdateInfo {
        skill_id: skill_id.to_string(),
        skill_name: skill_name.to_string(),
        has_update,
        remote_url: remote_url.to_string(),
        error: None,
    }
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Check whether one or more skills have updates available at their remote URL.
/// If `skill_ids` is None, checks all skills that have a `remote_url` stored.
#[tauri::command]
pub async fn check_skill_updates(
    state: State<'_, AppState>,
    skill_ids: Option<Vec<String>>,
) -> Result<Vec<SkillUpdateInfo>, String> {
    use sqlx::Row;

    let skills_to_check: Vec<(String, String, String, String)> = match skill_ids {
        Some(ids) => {
            let mut skills = Vec::with_capacity(ids.len());
            for id in ids {
                let row = sqlx::query(
                    "SELECT id, name, file_path, remote_url FROM skills WHERE id = ? AND remote_url IS NOT NULL",
                )
                .bind(&id)
                .fetch_optional(&state.db)
                .await
                .map_err(|e| e.to_string())?;

                if let Some(r) = row {
                    skills.push((
                        r.get::<String, _>("id"),
                        r.get::<String, _>("name"),
                        r.get::<String, _>("file_path"),
                        r.get::<String, _>("remote_url"),
                    ));
                }
            }
            skills
        }
        None => {
            let rows = sqlx::query(
                "SELECT id, name, file_path, remote_url FROM skills WHERE remote_url IS NOT NULL",
            )
            .fetch_all(&state.db)
            .await
            .map_err(|e| e.to_string())?;

            rows.into_iter()
                .map(|r| {
                    (
                        r.get::<String, _>("id"),
                        r.get::<String, _>("name"),
                        r.get::<String, _>("file_path"),
                        r.get::<String, _>("remote_url"),
                    )
                })
                .collect()
        }
    };

    if skills_to_check.is_empty() {
        return Ok(Vec::new());
    }

    let client = reqwest::Client::builder()
        .user_agent("skills-manage/0.9.1")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    // Limit concurrent downloads to 5 to avoid rate-limiting.
    let results: Vec<SkillUpdateInfo> = stream::iter(skills_to_check)
        .map(|(skill_id, skill_name, file_path, remote_url)| {
            let client = client.clone();
            async move {
                check_single_skill_update(&client, &skill_id, &skill_name, &file_path, &remote_url)
                    .await
            }
        })
        .buffer_unordered(5)
        .collect()
        .await;

    Ok(results)
}

/// Update a single skill by re-downloading its remote content and overwriting
/// the local SKILL.md. The skill must have a `remote_url` stored in the DB.
#[tauri::command]
pub async fn update_skill(
    state: State<'_, AppState>,
    skill_id: String,
) -> Result<(), String> {
    use sqlx::Row;

    let row = sqlx::query("SELECT file_path, remote_url FROM skills WHERE id = ?")
        .bind(&skill_id)
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Skill not found".to_string())?;

    let file_path: String = row.get("file_path");
    let remote_url: Option<String> = row.get("remote_url");

    let remote_url = remote_url.ok_or_else(|| "Skill has no remote URL".to_string())?;

    let client = reqwest::Client::builder()
        .user_agent("skills-manage/0.9.1")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let content = download_remote_text(&client, &remote_url).await?;

    // Write to local file
    std::fs::write(&file_path, content)
        .map_err(|e| format!("Failed to write skill file: {}", e))?;

    // Update scanned_at in DB
    let now = chrono::Utc::now().to_rfc3339();
    sqlx::query("UPDATE skills SET scanned_at = ? WHERE id = ?")
        .bind(&now)
        .bind(&skill_id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_update_info_serialization() {
        let info = SkillUpdateInfo {
            skill_id: "test-skill".to_string(),
            skill_name: "Test Skill".to_string(),
            has_update: true,
            remote_url: "https://example.com/SKILL.md".to_string(),
            error: None,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"hasUpdate\":true"));
        assert!(json.contains("\"skillId\":\"test-skill\""));
    }
}
