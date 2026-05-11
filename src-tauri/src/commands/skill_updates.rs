use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqlitePool};
use std::path::Path;
use std::time::Duration;
use tauri::State;

use crate::{commands::scanner::parse_skill_content, AppState};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FailedSkillUpdate {
    pub skill_id: String,
    pub skill_name: String,
    pub error: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSkillUpdateResult {
    pub updated: Vec<String>,
    pub skipped: Vec<String>,
    pub failed: Vec<FailedSkillUpdate>,
}

#[derive(Debug, Clone)]
struct SkillUpdateTarget {
    id: String,
    name: String,
    file_path: String,
    remote_url: String,
}

#[derive(Debug, Clone)]
enum SingleUpdateOutcome {
    Updated(String),
    Skipped(String),
    Failed(FailedSkillUpdate),
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

fn normalize_content(s: &str) -> String {
    s.replace("\r\n", "\n").trim().to_string()
}

async fn load_update_targets(
    pool: &SqlitePool,
    skill_ids: Option<Vec<String>>,
) -> Result<Vec<SkillUpdateTarget>, String> {
    let targets = match skill_ids {
        Some(ids) => {
            let mut skills = Vec::with_capacity(ids.len());
            for id in ids {
                let row = sqlx::query(
                    "SELECT id, name, file_path, remote_url FROM skills WHERE id = ? AND remote_url IS NOT NULL",
                )
                .bind(&id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?;

                if let Some(r) = row {
                    skills.push(SkillUpdateTarget {
                        id: r.get("id"),
                        name: r.get("name"),
                        file_path: r.get("file_path"),
                        remote_url: r.get("remote_url"),
                    });
                }
            }
            skills
        }
        None => {
            let rows = sqlx::query(
                "SELECT id, name, file_path, remote_url FROM skills WHERE remote_url IS NOT NULL",
            )
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

            rows.into_iter()
                .map(|r| SkillUpdateTarget {
                    id: r.get("id"),
                    name: r.get("name"),
                    file_path: r.get("file_path"),
                    remote_url: r.get("remote_url"),
                })
                .collect()
        }
    };

    Ok(targets)
}

fn write_skill_file(file_path: &str, content: &str) -> Result<(), String> {
    let path = Path::new(file_path);
    let tmp_path = path.with_file_name("SKILL.md.skills-manage-update.tmp");
    std::fs::write(&tmp_path, content)
        .map_err(|e| format!("Failed to write temporary skill file: {}", e))?;
    std::fs::rename(&tmp_path, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("Failed to replace skill file: {}", e)
    })
}

async fn update_skill_metadata(
    pool: &SqlitePool,
    target: &SkillUpdateTarget,
    content: &str,
) -> Result<(), String> {
    let info = parse_skill_content(content)
        .ok_or_else(|| "Remote SKILL.md has invalid or missing frontmatter".to_string())?;
    let now = chrono::Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE skills SET name = ?, description = ?, scanned_at = ? WHERE id = ?",
    )
    .bind(&info.name)
    .bind(&info.description)
    .bind(&now)
    .bind(&target.id)
    .execute(pool)
    .await
    .map_err(|e| e.to_string())?;

    Ok(())
}

async fn update_single_target(
    pool: &SqlitePool,
    client: &reqwest::Client,
    target: SkillUpdateTarget,
    skip_unchanged: bool,
) -> SingleUpdateOutcome {
    let remote_content = match download_remote_text(client, &target.remote_url).await {
        Ok(content) => content,
        Err(error) => {
            return SingleUpdateOutcome::Failed(FailedSkillUpdate {
                skill_id: target.id,
                skill_name: target.name,
                error,
            });
        }
    };

    if parse_skill_content(&remote_content).is_none() {
        return SingleUpdateOutcome::Failed(FailedSkillUpdate {
            skill_id: target.id,
            skill_name: target.name,
            error: "Remote SKILL.md has invalid or missing frontmatter".to_string(),
        });
    }

    if skip_unchanged {
        match std::fs::read_to_string(&target.file_path) {
            Ok(local_content)
                if normalize_content(&local_content) == normalize_content(&remote_content) =>
            {
                return SingleUpdateOutcome::Skipped(target.id);
            }
            Ok(_) => {}
            Err(error) => {
                return SingleUpdateOutcome::Failed(FailedSkillUpdate {
                    skill_id: target.id,
                    skill_name: target.name,
                    error: format!("Failed to read local file: {}", error),
                });
            }
        }
    }

    if let Err(error) = write_skill_file(&target.file_path, &remote_content) {
        return SingleUpdateOutcome::Failed(FailedSkillUpdate {
            skill_id: target.id,
            skill_name: target.name,
            error,
        });
    }

    match update_skill_metadata(pool, &target, &remote_content).await {
        Ok(_) => SingleUpdateOutcome::Updated(target.id),
        Err(error) => SingleUpdateOutcome::Failed(FailedSkillUpdate {
            skill_id: target.id,
            skill_name: target.name,
            error,
        }),
    }
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

    let has_update = normalize_content(&local_content) != normalize_content(&remote_content);

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
    let skills_to_check = load_update_targets(&state.db, skill_ids).await?;

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
        .map(|target| {
            let client = client.clone();
            async move {
                check_single_skill_update(
                    &client,
                    &target.id,
                    &target.name,
                    &target.file_path,
                    &target.remote_url,
                )
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
    let target = load_update_targets(&state.db, Some(vec![skill_id]))
        .await?
        .into_iter()
        .next()
        .ok_or_else(|| "Skill not found or has no remote URL".to_string())?;

    let client = reqwest::Client::builder()
        .user_agent("skills-manage/0.9.1")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    match update_single_target(&state.db, &client, target, false).await {
        SingleUpdateOutcome::Updated(_) | SingleUpdateOutcome::Skipped(_) => Ok(()),
        SingleUpdateOutcome::Failed(failure) => Err(failure.error),
    }
}

/// Update every selected skill that has changed remotely. If `skill_ids` is
/// None, all skills with a stored remote URL are considered.
#[tauri::command]
pub async fn update_skills(
    state: State<'_, AppState>,
    skill_ids: Option<Vec<String>>,
) -> Result<BatchSkillUpdateResult, String> {
    let targets = load_update_targets(&state.db, skill_ids).await?;

    if targets.is_empty() {
        return Ok(BatchSkillUpdateResult {
            updated: Vec::new(),
            skipped: Vec::new(),
            failed: Vec::new(),
        });
    }

    let client = reqwest::Client::builder()
        .user_agent("skills-manage/0.9.1")
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let results: Vec<SingleUpdateOutcome> = stream::iter(targets)
        .map(|target| {
            let client = client.clone();
            let pool = state.db.clone();
            async move { update_single_target(&pool, &client, target, true).await }
        })
        .buffer_unordered(3)
        .collect()
        .await;

    let mut result = BatchSkillUpdateResult {
        updated: Vec::new(),
        skipped: Vec::new(),
        failed: Vec::new(),
    };

    for outcome in results {
        match outcome {
            SingleUpdateOutcome::Updated(skill_id) => result.updated.push(skill_id),
            SingleUpdateOutcome::Skipped(skill_id) => result.skipped.push(skill_id),
            SingleUpdateOutcome::Failed(failure) => result.failed.push(failure),
        }
    }

    Ok(result)
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
