use serde::{Deserialize, Serialize};
use std::time::Duration;

const GITHUB_REPO: &str = "iamzhihuix/skills-manage";
const GITHUB_API_URL: &str = "https://api.github.com";

// ─── Types ────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub release_notes: String,
    pub published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    body: Option<String>,
    published_at: Option<String>,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn current_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Compare two semantic version strings (e.g. "0.10.0" vs "0.9.1").
/// Strips a leading 'v' if present.
fn compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> Vec<u32> {
        s.trim_start_matches('v')
            .split('.')
            .map(|p| p.parse::<u32>().unwrap_or(0))
            .collect()
    };
    let av = parse(a);
    let bv = parse(b);
    for i in 0..av.len().max(bv.len()) {
        let avc = av.get(i).copied().unwrap_or(0);
        let bvc = bv.get(i).copied().unwrap_or(0);
        match avc.cmp(&bvc) {
            std::cmp::Ordering::Equal => continue,
            other => return other,
        }
    }
    std::cmp::Ordering::Equal
}

// ─── Commands ─────────────────────────────────────────────────────────────────

/// Check whether a newer release exists on GitHub.
#[tauri::command]
pub async fn check_update() -> Result<UpdateCheckResult, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let url = format!("{}/repos/{}/releases/latest", GITHUB_API_URL, GITHUB_REPO);
    let response = client
        .get(&url)
        .header("User-Agent", "skills-manage-updater")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("Failed to fetch latest release: {}", e))?;

    let status = response.status();
    if status == 404 {
        return Err("No releases found for this repository.".to_string());
    }
    if status == 403 {
        return Err(
            "GitHub API rate limit hit. Please try again later.".to_string(),
        );
    }
    if !status.is_success() {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "(unable to read body)".to_string());
        return Err(format!("GitHub API error ({}): {}", status, body));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| format!("Failed to parse release info: {}", e))?;

    let current = current_version();
    let latest = release.tag_name.trim_start_matches('v').to_string();

    let has_update = compare_versions(&latest, &current) == std::cmp::Ordering::Greater;

    Ok(UpdateCheckResult {
        has_update,
        current_version: current,
        latest_version: latest,
        release_url: release.html_url,
        release_notes: release.body.unwrap_or_default(),
        published_at: release.published_at,
    })
}

/// Return the current application version from Cargo.toml.
#[tauri::command]
pub fn get_app_version() -> String {
    current_version()
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_versions_basic() {
        assert_eq!(compare_versions("0.10.0", "0.9.1"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("0.9.1", "0.10.0"), std::cmp::Ordering::Less);
        assert_eq!(compare_versions("1.2.3", "1.2.3"), std::cmp::Ordering::Equal);
    }

    #[test]
    fn test_compare_versions_with_v_prefix() {
        assert_eq!(compare_versions("v0.10.0", "0.9.1"), std::cmp::Ordering::Greater);
        assert_eq!(compare_versions("0.10.0", "v0.9.1"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_compare_versions_different_length() {
        assert_eq!(compare_versions("1.0", "1.0.0"), std::cmp::Ordering::Equal);
        assert_eq!(compare_versions("1.0.1", "1.0"), std::cmp::Ordering::Greater);
    }

    #[test]
    fn test_current_version_not_empty() {
        let v = current_version();
        assert!(!v.is_empty(), "Current version should not be empty");
    }
}
