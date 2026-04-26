use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json;
use tauri::{Emitter, State};

use crate::db::{self, DbPool};
use crate::path_utils::{central_skills_dir, path_to_string, resolve_home_dir};
use crate::AppState;

const OBSIDIAN_PLATFORM_ID: &str = "obsidian";
const OBSIDIAN_PLATFORM_NAME: &str = "Obsidian";

// ─── Types ────────────────────────────────────────────────────────────────────

/// A candidate scan root (e.g. ~/projects, ~/Developer).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanRoot {
    pub path: String,
    pub label: String,
    pub exists: bool,
    pub enabled: bool,
}

/// A project-level skill discovered during a full-disk scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredSkill {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub file_path: String,
    pub dir_path: String,
    pub platform_id: String,
    pub platform_name: String,
    pub project_path: String,
    pub project_name: String,
    /// True if this skill already exists in the central skills dir.
    pub is_already_central: bool,
}

/// A project that contains skills, grouped for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProject {
    pub project_path: String,
    pub project_name: String,
    pub skills: Vec<DiscoveredSkill>,
}

/// Payload emitted during scan for progress updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub percent: u32,
    pub current_path: String,
    pub skills_found: usize,
    pub projects_found: usize,
}

/// Payload emitted when a project with skills is found.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FoundPayload {
    pub project: DiscoveredProject,
}

/// Payload emitted when scan completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletePayload {
    pub total_projects: usize,
    pub total_skills: usize,
}

/// Result of the full project scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoverResult {
    pub total_projects: usize,
    pub total_skills: usize,
    pub projects: Vec<DiscoveredProject>,
}

/// Target for importing a discovered skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImportTarget {
    Central,
    Platform(String),
}

/// Result of importing a discovered skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub skill_id: String,
    pub target: String,
}

// ─── Global cancel flag ──────────────────────────────────────────────────────

static SCAN_CANCEL: AtomicBool = AtomicBool::new(false);

// ─── Default scan roots ───────────────────────────────────────────────────────

/// Returns a list of candidate scan roots, checking which ones exist on disk.
fn default_scan_roots() -> Vec<ScanRoot> {
    let home = resolve_home_dir();
    default_scan_roots_for_home(&home)
}

fn default_scan_roots_for_home(home: &Path) -> Vec<ScanRoot> {
    let candidates = vec![
        (path_to_string(&home.join("projects")), "projects"),
        (path_to_string(&home.join("Documents")), "Documents"),
        (path_to_string(&home.join("Developer")), "Developer"),
        (path_to_string(&home.join("work")), "work"),
        (path_to_string(&home.join("src")), "src"),
        (path_to_string(&home.join("code")), "code"),
        (path_to_string(&home.join("repos")), "repos"),
        (path_to_string(&home.join("Desktop")), "Desktop"),
        (
            path_to_string(
                &home
                    .join("Library")
                    .join("Mobile Documents")
                    .join("iCloud~md~obsidian")
                    .join("Documents"),
            ),
            "Obsidian iCloud",
        ),
        // macOS: scan /Applications for apps with built-in skills (e.g. OpenClaw)
        ("/Applications".to_string(), "Applications"),
    ];

    candidates
        .into_iter()
        .map(|(path, label)| {
            let exists = Path::new(&path).exists();
            ScanRoot {
                path,
                label: label.to_string(),
                exists,
                enabled: exists, // auto-enable roots that exist
            }
        })
        .collect()
}

fn normalized_scan_root_key(path: &str) -> String {
    if let Ok(canonical) = std::fs::canonicalize(path) {
        return path_to_string(&canonical);
    }

    let trimmed = path.trim();
    let without_trailing = trimmed.trim_end_matches(['/', '\\']);
    if without_trailing.is_empty() {
        trimmed.to_string()
    } else {
        without_trailing.to_string()
    }
}

fn label_for_custom_scan_root(path: &str, label: Option<&str>) -> String {
    label
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.to_string())
        .or_else(|| {
            Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| name.to_string())
        })
        .unwrap_or_else(|| path.to_string())
}

async fn build_scan_roots(pool: &DbPool, defaults: Vec<ScanRoot>) -> Result<Vec<ScanRoot>, String> {
    let mut roots = defaults;
    let mut seen_paths: HashSet<String> = roots
        .iter()
        .map(|root| normalized_scan_root_key(&root.path))
        .collect();

    let mut custom_dirs: Vec<_> = db::get_scan_directories(pool)
        .await?
        .into_iter()
        .filter(|dir| !dir.is_builtin)
        .collect();
    custom_dirs.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then_with(|| a.label.cmp(&b.label))
            .then_with(|| a.id.cmp(&b.id))
    });

    for dir in custom_dirs {
        let key = normalized_scan_root_key(&dir.path);
        if !seen_paths.insert(key) {
            continue;
        }

        roots.push(ScanRoot {
            path: dir.path.clone(),
            label: label_for_custom_scan_root(&dir.path, dir.label.as_deref()),
            exists: Path::new(&dir.path).exists(),
            enabled: dir.is_active,
        });
    }

    // Load persisted enabled states from settings.
    // We store a single JSON blob under the key "discover_scan_roots_config"
    // mapping path -> enabled (bool). This override is applied after default
    // and custom roots are merged so duplicate paths get one deterministic state.
    if let Some(json) = db::get_setting(pool, "discover_scan_roots_config").await? {
        let config: HashMap<String, bool> =
            serde_json::from_str(&json).map_err(|e| format!("Invalid scan roots config: {}", e))?;
        for root in &mut roots {
            if let Some(&enabled) = config.get(&root.path) {
                root.enabled = enabled;
            }
        }
    }

    Ok(roots)
}

async fn get_scan_roots_impl(pool: &DbPool) -> Result<Vec<ScanRoot>, String> {
    build_scan_roots(pool, default_scan_roots()).await
}

/// Build the list of platform skill directory patterns to look for.
/// For each enabled agent, its `global_skills_dir` is split to derive a
/// relative pattern like `.claude/skills` from `/home/user/.claude/skills`.
fn platform_skill_patterns(_pool: &DbPool) -> Vec<(String, String, PathBuf)> {
    // (agent_id, display_name, relative_subpath)
    // We compute this synchronously since it only reads from the built-in
    // agent list which is static after init.
    let home = resolve_home_dir();

    db::builtin_agents()
        .iter()
        .filter(|a| a.id != "central")
        .filter_map(|a| {
            let full = PathBuf::from(&a.global_skills_dir);
            // Strip home prefix to get relative path like ".claude/skills"
            let rel = full.strip_prefix(&home).ok()?;
            Some((a.id.clone(), a.display_name.clone(), rel.to_path_buf()))
        })
        .collect()
}

// ─── Core scan logic ──────────────────────────────────────────────────────────

/// Maximum recursion depth for the directory walker.
const MAX_SCAN_DEPTH: u32 = 8;

/// Directory names that should always be skipped during traversal
/// for performance (these never contain project-level skill dirs).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "target",
    ".git",
    "build",
    "dist",
    ".cache",
    "__pycache__",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".idea",
    ".vscode",
];

/// Check whether a directory name should be skipped during traversal.
///
/// - Always skip known heavy/irrelevant directories (node_modules, .git, etc.).
/// - At the root level (depth 0), skip hidden directories (dot-prefixed) since
///   they are typically user config dirs, not project directories.
/// - At deeper levels, allow hidden directories so we can detect platform
///   skill patterns like `.claude/skills/` inside project dirs.
fn should_skip_dir(name: &str, depth: u32) -> bool {
    // Always skip known heavy directories.
    if SKIP_DIRS.contains(&name) {
        return true;
    }

    // At root level, skip hidden directories (dot-prefixed).
    // These are typically user config dirs (~/.config, ~/.local, etc.),
    // not project directories containing skills.
    if depth == 0 && name.starts_with('.') {
        return true;
    }

    false
}

fn is_obsidian_vault_dir(path: &Path) -> bool {
    path.join(".obsidian").is_dir()
}

fn file_name_or_unknown(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn stable_path_hash(path: &str) -> String {
    // FNV-1a gives us a deterministic, compact path-derived component without
    // adding a dependency or relying on randomized hash seeds.
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{hash:016x}")
}

fn selected_skill_dir_name(dir_path: &str) -> String {
    Path::new(dir_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn obsidian_qualified_id(vault_path: &str, skill_id: &str) -> String {
    format!(
        "{}__{}__{}",
        OBSIDIAN_PLATFORM_ID,
        stable_path_hash(vault_path),
        skill_id
    )
}

fn platform_display_name(platform_id: &str) -> String {
    if platform_id == OBSIDIAN_PLATFORM_ID {
        return OBSIDIAN_PLATFORM_NAME.to_string();
    }

    db::builtin_agents()
        .iter()
        .find(|agent| agent.id == platform_id)
        .map(|agent| agent.display_name.clone())
        .unwrap_or_else(|| platform_id.to_string())
}

fn scan_obsidian_vault(vault_dir: &Path, central_dir: &Path) -> Option<DiscoveredProject> {
    if !is_obsidian_vault_dir(vault_dir) {
        return None;
    }

    let project_path = path_to_string(vault_dir);
    let project_name = file_name_or_unknown(vault_dir);
    let mut selected_by_skill_id: BTreeMap<String, DiscoveredSkill> = BTreeMap::new();

    // Priority order is intentional: .skills wins over .agents/skills, which
    // wins over .claude/skills. Invalid directories are skipped by
    // scanner::scan_directory and therefore never reserve a dedupe key.
    for rel_source in [
        PathBuf::from(".skills"),
        PathBuf::from(".agents/skills"),
        PathBuf::from(".claude/skills"),
    ] {
        let source_dir = vault_dir.join(rel_source);
        let mut scanned = super::scanner::scan_directory(&source_dir, false);
        scanned.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.dir_path.cmp(&b.dir_path)));

        for skill in scanned {
            if selected_by_skill_id.contains_key(&skill.id) {
                continue;
            }

            let skill_dir_name = selected_skill_dir_name(&skill.dir_path);
            let is_already_central = central_dir.join(skill_dir_name).exists();
            selected_by_skill_id.insert(
                skill.id.clone(),
                DiscoveredSkill {
                    id: obsidian_qualified_id(&project_path, &skill.id),
                    name: skill.name,
                    description: skill.description,
                    file_path: skill.file_path,
                    dir_path: skill.dir_path,
                    platform_id: OBSIDIAN_PLATFORM_ID.to_string(),
                    platform_name: OBSIDIAN_PLATFORM_NAME.to_string(),
                    project_path: project_path.clone(),
                    project_name: project_name.clone(),
                    is_already_central,
                },
            );
        }
    }

    if selected_by_skill_id.is_empty() {
        None
    } else {
        Some(DiscoveredProject {
            project_path,
            project_name,
            skills: selected_by_skill_id.into_values().collect(),
        })
    }
}

fn scan_regular_project_dir(
    project_dir: &Path,
    patterns: &[(String, String, PathBuf)],
    central_dir: &Path,
) -> Vec<DiscoveredSkill> {
    let mut project_skills: Vec<DiscoveredSkill> = Vec::new();

    for (agent_id, display_name, rel_pattern) in patterns {
        let skill_dir = project_dir.join(rel_pattern);

        if !skill_dir.exists() {
            continue;
        }

        let scanned = super::scanner::scan_directory(&skill_dir, false);

        for skill in scanned {
            // Preserve the established ordinary Discover row identity. Obsidian
            // uses path-hashed IDs because duplicate vault basenames are a
            // mission requirement, but changing ordinary IDs would strand
            // existing cached Discover rows.
            let project_name = file_name_or_unknown(project_dir);
            let project_path = project_dir.to_string_lossy().into_owned();

            let qualified_id = format!(
                "{}__{}__{}",
                agent_id,
                project_name.to_lowercase().replace(' ', "-"),
                skill.id
            );

            // Check if this skill already exists in central.
            let skill_dir_name = selected_skill_dir_name(&skill.dir_path);
            let central_skill_path = central_dir.join(skill_dir_name);
            let is_already_central = central_skill_path.exists();

            project_skills.push(DiscoveredSkill {
                id: qualified_id,
                name: skill.name,
                description: skill.description,
                file_path: skill.file_path,
                dir_path: skill.dir_path,
                platform_id: agent_id.clone(),
                platform_name: display_name.clone(),
                project_path,
                project_name,
                is_already_central,
            });
        }
    }

    project_skills
}

/// Recursively walk a scan root directory, looking for project-level skill dirs.
///
/// Traverses subdirectories up to `MAX_SCAN_DEPTH` levels deep, checking each
/// directory for known platform skill subdirectories (e.g., `.claude/skills/`,
/// `.cursor/skills/`, `.factory/skills/`). When a match is found, the
/// containing directory is treated as a "project" and its skills are collected.
///
/// Skips hidden directories at root level (except dot-prefixed platform dirs
/// which are matched via patterns), and always skips performance-heavy
/// directories like `node_modules`, `.git`, `target`, `build`, `dist`.
///
/// The `project_path` is the directory that CONTAINS the platform dir
/// (e.g., `~/Documents/GitHubMe/minimax-skills` for `.claude/skills/` found there).
#[cfg(test)]
fn scan_root_for_projects(
    root: &Path,
    patterns: &[(String, String, PathBuf)],
    central_dir: &Path,
) -> Vec<DiscoveredProject> {
    let mut projects = Vec::new();
    let mut seen_project_paths = HashSet::new();
    scan_root_for_projects_with_seen(
        root,
        patterns,
        central_dir,
        &mut seen_project_paths,
        &mut projects,
    );
    projects
}

fn scan_root_for_projects_with_seen(
    root: &Path,
    patterns: &[(String, String, PathBuf)],
    central_dir: &Path,
    seen_project_paths: &mut HashSet<String>,
    projects: &mut Vec<DiscoveredProject>,
) {
    scan_root_recursive(root, patterns, central_dir, 0, projects, seen_project_paths);
}

/// Inner recursive walker. Accumulates found projects into `projects`.
/// `seen_project_paths` prevents duplicates when the same project dir
/// is reached via different scan roots.
fn scan_root_recursive(
    current_dir: &Path,
    patterns: &[(String, String, PathBuf)],
    central_dir: &Path,
    depth: u32,
    projects: &mut Vec<DiscoveredProject>,
    seen_project_paths: &mut HashSet<String>,
) {
    if depth > MAX_SCAN_DEPTH {
        return;
    }
    if SCAN_CANCEL.load(Ordering::Relaxed) {
        return;
    }

    let current_path_key = current_dir.to_string_lossy().into_owned();
    if !seen_project_paths.contains(&current_path_key) {
        if let Some(project) = scan_obsidian_vault(current_dir, central_dir) {
            seen_project_paths.insert(current_path_key);
            projects.push(project);
            // A vault is a single Discover project. Do not also treat its
            // `.agents/skills` or `.claude/skills` directories as ordinary
            // platform projects.
            return;
        }

        let project_skills = scan_regular_project_dir(current_dir, patterns, central_dir);
        if !project_skills.is_empty() {
            seen_project_paths.insert(current_path_key.clone());
            projects.push(DiscoveredProject {
                project_path: current_path_key,
                project_name: file_name_or_unknown(current_dir),
                skills: project_skills,
            });
        }
    }

    let entries = match std::fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        if SCAN_CANCEL.load(Ordering::Relaxed) {
            break;
        }

        let entry_path = entry.path();

        // Only look at directories.
        let meta = match std::fs::metadata(&entry_path) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !meta.is_dir() {
            continue;
        }

        let dir_name = entry_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        // Skip directories that should never be traversed.
        if should_skip_dir(dir_name, depth) {
            continue;
        }

        scan_root_recursive(
            &entry_path,
            patterns,
            central_dir,
            depth + 1,
            projects,
            seen_project_paths,
        );
    }
}

// ─── Cache Reconciliation ─────────────────────────────────────────────────────

/// Reconcile the `discovered_skills` table after a scan.
///
/// For each previously discovered skill whose `project_path` falls under one of
/// the scanned roots, check whether the skill's `dir_path` still exists on disk.
/// If not, delete the stale record from the database.
///
/// Skills that were found during the current scan (identified by `found_skill_ids`)
/// are always kept — they are fresh and valid.
async fn reconcile_discovered_skills(
    pool: &DbPool,
    scan_roots: &[&ScanRoot],
    found_skill_ids: &[String],
) -> Result<(), String> {
    let all_rows = db::get_all_discovered_skills(pool).await?;

    let found_set: std::collections::HashSet<&str> =
        found_skill_ids.iter().map(|s| s.as_str()).collect();

    for row in &all_rows {
        // Skip skills just found in this scan — they are valid.
        if found_set.contains(row.id.as_str()) {
            continue;
        }

        // Only reconcile skills under the scanned roots.
        let project_path = Path::new(&row.project_path);
        let under_scanned_root = scan_roots.iter().any(|root| {
            project_path.starts_with(&root.path)
                || project_path.as_os_str() == std::ffi::OsStr::new(&root.path)
        });

        if !under_scanned_root {
            continue;
        }

        // Obsidian rows are authoritative for the scanned vault scope: if a
        // rescan did not emit the row, it may have lost validity, priority, or
        // its `.obsidian` marker even if the old directory still exists. For
        // ordinary rows, preserve the prior conservative behavior and only
        // purge directories that are actually gone.
        let should_delete = if row.platform_id == OBSIDIAN_PLATFORM_ID {
            true
        } else {
            !Path::new(&row.dir_path).exists()
        };

        if should_delete {
            db::delete_discovered_skill(pool, &row.id).await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
enum DiscoverEvent {
    Found(FoundPayload),
    Progress(ProgressPayload),
    Complete(CompletePayload),
}

async fn start_project_scan_impl<F>(
    pool: &DbPool,
    roots: Vec<ScanRoot>,
    central_dir: &Path,
    mut emit_event: F,
) -> Result<DiscoverResult, String>
where
    F: FnMut(DiscoverEvent),
{
    // Build platform skill patterns from registered agents.
    let patterns = platform_skill_patterns(pool);

    // Filter to enabled roots that exist.
    let enabled_roots: Vec<&ScanRoot> = roots.iter().filter(|r| r.enabled && r.exists).collect();
    let total_roots = enabled_roots.len();

    let mut all_projects: Vec<DiscoveredProject> = Vec::new();
    let mut total_skills = 0;
    let mut roots_scanned = 0;
    let mut seen_project_paths = HashSet::new();

    for root in &enabled_roots {
        if SCAN_CANCEL.load(Ordering::Relaxed) {
            break;
        }

        let root_path = Path::new(&root.path);
        let before_project_count = all_projects.len();
        scan_root_for_projects_with_seen(
            root_path,
            &patterns,
            central_dir,
            &mut seen_project_paths,
            &mut all_projects,
        );
        let found_projects: Vec<DiscoveredProject> = all_projects[before_project_count..].to_vec();

        roots_scanned += 1;
        let percent = if total_roots > 0 {
            (roots_scanned as u32 * 100) / total_roots as u32
        } else {
            100
        };

        for project in &found_projects {
            total_skills += project.skills.len();

            emit_event(DiscoverEvent::Found(FoundPayload {
                project: project.clone(),
            }));
        }

        emit_event(DiscoverEvent::Progress(ProgressPayload {
            percent: percent.min(100),
            current_path: root.path.clone(),
            skills_found: total_skills,
            projects_found: all_projects.len(),
        }));
    }

    // Persist discovered skills to the database.
    let now = Utc::now().to_rfc3339();

    // Collect all discovered skill IDs found in this scan for reconciliation.
    let mut found_skill_ids: Vec<String> = Vec::new();

    for project in &all_projects {
        for skill in &project.skills {
            found_skill_ids.push(skill.id.clone());

            db::insert_discovered_skill(
                pool,
                &skill.id,
                &skill.name,
                skill.description.as_deref(),
                &skill.file_path,
                &skill.dir_path,
                &skill.project_path,
                &skill.project_name,
                &skill.platform_id,
                &now,
            )
            .await?;
        }
    }

    // ── Cache reconciliation ──────────────────────────────────────────────────
    // Remove stale discovered_skills rows within the scanned scope.
    reconcile_discovered_skills(pool, &enabled_roots, &found_skill_ids).await?;

    let total_projects = all_projects.len();

    emit_event(DiscoverEvent::Complete(CompletePayload {
        total_projects,
        total_skills,
    }));

    Ok(DiscoverResult {
        total_projects,
        total_skills,
        projects: all_projects,
    })
}

// ─── Tauri Commands ───────────────────────────────────────────────────────────

/// Auto-detect candidate scan roots and return them.
#[tauri::command]
pub async fn discover_scan_roots() -> Result<Vec<ScanRoot>, String> {
    Ok(default_scan_roots())
}

/// Get scan roots with persisted enabled state from DB.
///
/// Returns auto-detected default roots, then overlays any previously
/// persisted enabled/disabled states from the settings table.
#[tauri::command]
pub async fn get_scan_roots(state: State<'_, AppState>) -> Result<Vec<ScanRoot>, String> {
    get_scan_roots_impl(&state.db).await
}

/// Persist the enabled/disabled state of a scan root.
///
/// Updates the "discover_scan_roots_config" setting in the DB, which
/// stores a JSON object mapping root paths to their enabled state.
#[tauri::command]
pub async fn set_scan_root_enabled(
    state: State<'_, AppState>,
    path: String,
    enabled: bool,
) -> Result<(), String> {
    let pool = &state.db;

    // Load existing config or start fresh.
    let mut config: HashMap<String, bool> =
        match db::get_setting(pool, "discover_scan_roots_config").await? {
            Some(json) => serde_json::from_str(&json)
                .map_err(|e| format!("Invalid scan roots config: {}", e))?,
            None => HashMap::new(),
        };

    config.insert(path, enabled);

    let json = serde_json::to_string(&config)
        .map_err(|e| format!("Failed to serialize scan roots config: {}", e))?;
    db::set_setting(pool, "discover_scan_roots_config", &json).await
}

/// Start a project-discovery scan across the given root directories.
/// Emits streaming events (`discover:progress`, `discover:found`, `discover:complete`).
#[tauri::command]
pub async fn start_project_scan(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    roots: Vec<ScanRoot>,
) -> Result<DiscoverResult, String> {
    // Reset cancel flag.
    SCAN_CANCEL.store(false, Ordering::Relaxed);

    let pool = &state.db;
    let central_dir = central_skills_dir();
    start_project_scan_impl(pool, roots, &central_dir, |event| match event {
        DiscoverEvent::Found(payload) => {
            let _ = app.emit("discover:found", payload);
        }
        DiscoverEvent::Progress(payload) => {
            let _ = app.emit("discover:progress", payload);
        }
        DiscoverEvent::Complete(payload) => {
            let _ = app.emit("discover:complete", payload);
        }
    })
    .await
}

/// Cancel an in-progress project scan.
#[tauri::command]
pub async fn stop_project_scan() -> Result<(), String> {
    SCAN_CANCEL.store(true, Ordering::Relaxed);
    Ok(())
}

/// Load previously discovered skills from the database, grouped by project.
#[tauri::command]
pub async fn get_discovered_skills(
    state: State<'_, AppState>,
) -> Result<Vec<DiscoveredProject>, String> {
    let central_dir = central_skills_dir();
    get_discovered_skills_impl(&state.db, &central_dir).await
}

async fn get_discovered_skills_impl(
    pool: &DbPool,
    central_dir: &Path,
) -> Result<Vec<DiscoveredProject>, String> {
    let rows = db::get_all_discovered_skills(pool).await?;

    // Convert DB rows to DiscoveredSkill structs, adding is_already_central.
    let skills: Vec<DiscoveredSkill> = rows
        .into_iter()
        .map(|row| {
            let skill_dir_name = Path::new(&row.dir_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown");
            // A discovered skill is "already central" if:
            // 1. The skill directory name exists in the central skills dir, OR
            // 2. There is a skill_installations record for this skill name
            //    (meaning it's been installed to at least one platform).
            let is_already_central = central_dir.join(skill_dir_name).exists();
            let platform_id = row.platform_id.clone();

            DiscoveredSkill {
                id: row.id,
                name: row.name,
                description: row.description,
                file_path: row.file_path,
                dir_path: row.dir_path,
                platform_id: platform_id.clone(),
                platform_name: platform_display_name(&platform_id),
                project_path: row.project_path,
                project_name: row.project_name,
                is_already_central,
            }
        })
        .collect();

    // Group skills by project_path.
    let mut by_project: HashMap<String, Vec<DiscoveredSkill>> = HashMap::new();
    let mut project_names: HashMap<String, String> = HashMap::new();

    for skill in skills {
        project_names.insert(skill.project_path.clone(), skill.project_name.clone());
        by_project
            .entry(skill.project_path.clone())
            .or_default()
            .push(skill);
    }

    let mut projects: Vec<DiscoveredProject> = by_project
        .into_iter()
        .map(|(path, skills)| DiscoveredProject {
            project_path: path.clone(),
            project_name: project_names.get(&path).cloned().unwrap_or_default(),
            skills,
        })
        .collect();

    // Sort by project name for stable ordering.
    projects.sort_by(|a, b| a.project_name.cmp(&b.project_name));

    Ok(projects)
}

/// Import a discovered skill to the central skills directory.
///
/// Copies the skill directory from its project location to `~/.agents/skills/<skill_dir_name>`,
/// then records it in the skills table.
#[tauri::command]
pub async fn import_discovered_skill_to_central(
    state: State<'_, AppState>,
    discovered_skill_id: String,
) -> Result<ImportResult, String> {
    let pool = &state.db;

    // Look up the discovered skill.
    let skill = db::get_discovered_skill_by_id(pool, &discovered_skill_id)
        .await?
        .ok_or_else(|| format!("Discovered skill '{}' not found", discovered_skill_id))?;

    // Determine central dir.
    let central_dir = central_skills_dir();

    // Extract the original skill directory name (last component of dir_path).
    let skill_dir_name = Path::new(&skill.dir_path)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Cannot extract skill directory name".to_string())?
        .to_string();

    let target_dir = central_dir.join(&skill_dir_name);

    // Check if a skill with this name already exists in central.
    if target_dir.exists() {
        return Err(format!(
            "A skill named '{}' already exists in central skills",
            skill_dir_name
        ));
    }

    // Copy the skill directory to central.
    super::linker::copy_dir_all(Path::new(&skill.dir_path), &target_dir)?;

    // Now we need to re-scan so the new central skill gets picked up.
    // Record the skill in the DB as a central skill.
    let skill_md_path = target_dir.join("SKILL.md");
    let info = super::scanner::parse_skill_md(&skill_md_path);

    if let Some(skill_info) = info {
        let now = Utc::now().to_rfc3339();
        let db_skill = db::Skill {
            id: skill_dir_name.clone(),
            name: skill_info.name,
            description: skill_info.description,
            file_path: skill_md_path.to_string_lossy().into_owned(),
            canonical_path: Some(target_dir.to_string_lossy().into_owned()),
            is_central: true,
            source: Some("copy".to_string()),
            content: None,
            scanned_at: now,
        };
        db::upsert_skill(pool, &db_skill).await?;
    }

    // Remove the discovered skill record since it's now centralized.
    db::delete_discovered_skill(pool, &discovered_skill_id).await?;

    Ok(ImportResult {
        skill_id: skill_dir_name,
        target: "central".to_string(),
    })
}

/// Import a discovered skill to a specific platform's global skills directory.
///
/// Creates a symlink (or copy) from the discovered skill's dir to the platform's
/// global skills directory.
#[tauri::command]
pub async fn import_discovered_skill_to_platform(
    state: State<'_, AppState>,
    discovered_skill_id: String,
    agent_id: String,
) -> Result<ImportResult, String> {
    import_discovered_skill_to_platform_from_pool(&state.db, &discovered_skill_id, &agent_id).await
}

async fn import_discovered_skill_to_platform_from_pool(
    pool: &DbPool,
    discovered_skill_id: &str,
    agent_id: &str,
) -> Result<ImportResult, String> {
    if agent_id == OBSIDIAN_PLATFORM_ID {
        return Err("Obsidian vaults are Discover sources, not install targets".to_string());
    }

    // Look up the discovered skill.
    let skill = db::get_discovered_skill_by_id(pool, discovered_skill_id)
        .await?
        .ok_or_else(|| format!("Discovered skill '{}' not found", discovered_skill_id))?;

    // Look up the target agent.
    let agent = db::get_agent_by_id(pool, agent_id)
        .await?
        .ok_or_else(|| format!("Agent '{}' not found", agent_id))?;

    // Extract the original skill directory name.
    let skill_dir_name = Path::new(&skill.dir_path)
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Cannot extract skill directory name".to_string())?
        .to_string();

    let agent_dir = PathBuf::from(&agent.global_skills_dir);
    let target_path = agent_dir.join(&skill_dir_name);

    // Ensure agent skills dir exists.
    std::fs::create_dir_all(&agent_dir)
        .map_err(|e| format!("Failed to create agent skills directory: {}", e))?;

    // Check if already installed.
    if target_path.exists() || std::fs::symlink_metadata(&target_path).is_ok() {
        return Err(format!(
            "Skill '{}' already exists in {}",
            skill_dir_name, agent.display_name
        ));
    }

    // Create symlink from discovered skill dir to platform dir.
    let src_path = Path::new(&skill.dir_path);
    let relative_target = super::linker::symlink_target_path(&agent_dir, src_path);
    super::linker::create_symlink(&relative_target, &target_path)?;

    // Record the installation.
    let now = Utc::now().to_rfc3339();

    // Also ensure the skill is in the skills table.
    let skill_md_path = src_path.join("SKILL.md");
    let info = super::scanner::parse_skill_md(&skill_md_path);

    if let Some(skill_info) = info {
        let db_skill = db::Skill {
            id: skill_dir_name.clone(),
            name: skill_info.name,
            description: skill_info.description,
            file_path: skill_md_path.to_string_lossy().into_owned(),
            canonical_path: None,
            is_central: false,
            source: Some("symlink".to_string()),
            content: None,
            scanned_at: now.clone(),
        };
        db::upsert_skill(pool, &db_skill).await?;
    }

    let installation = db::SkillInstallation {
        skill_id: skill_dir_name.clone(),
        agent_id: agent_id.to_string(),
        installed_path: target_path.to_string_lossy().into_owned(),
        link_type: "symlink".to_string(),
        symlink_target: Some(skill.dir_path.clone()),
        created_at: now,
    };
    db::upsert_skill_installation(pool, &installation).await?;

    // NOTE: We intentionally do NOT delete the discovered skill record here.
    // Keeping the record allows multi-platform install (importing the same
    // discovered skill to multiple platforms in sequence). The record will be
    // cleaned up by cache reconciliation on the next rescan, or when the user
    // imports it to central (which does delete the record).

    Ok(ImportResult {
        skill_id: skill_dir_name,
        target: agent_id.to_string(),
    })
}

/// Clear all discovered skills from the database.
#[tauri::command]
pub async fn clear_discovered_skills(state: State<'_, AppState>) -> Result<(), String> {
    db::clear_all_discovered_skills(&state.db).await
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_scan_roots_returns_candidates() {
        let roots = default_scan_roots();
        assert!(!roots.is_empty(), "should return at least some candidates");
        // Each root should have a path and label.
        for root in &roots {
            assert!(!root.path.is_empty());
            assert!(!root.label.is_empty());
        }
    }

    #[test]
    fn test_scan_root_exists_matches_filesystem() {
        let roots = default_scan_roots();
        for root in &roots {
            let actually_exists = Path::new(&root.path).exists();
            assert_eq!(
                root.exists, actually_exists,
                "exists flag should match actual filesystem for {}",
                root.path
            );
        }
    }

    #[tokio::test]
    async fn test_platform_skill_patterns_excludes_central() {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        db::init_database(&pool).await.unwrap();
        let patterns = platform_skill_patterns(&pool);
        // Central should be excluded.
        assert!(
            !patterns.iter().any(|(id, _, _)| id == "central"),
            "central should not appear in platform skill patterns"
        );
        // Claude Code should be included.
        assert!(
            patterns.iter().any(|(id, _, _)| id == "claude-code"),
            "claude-code should appear in platform skill patterns"
        );
    }

    use sqlx::SqlitePool;

    fn valid_skill_md(name: &str, description: &str) -> String {
        format!(
            "---\nname: {}\ndescription: {}\n---\n\n# {}\n",
            name, description, name
        )
    }

    fn create_skill(parent: &Path, dir_name: &str, name: &str, description: &str) -> PathBuf {
        let skill_dir = parent.join(dir_name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            valid_skill_md(name, description),
        )
        .unwrap();
        skill_dir
    }

    fn create_invalid_skill(parent: &Path, dir_name: &str) -> PathBuf {
        let skill_dir = parent.join(dir_name);
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: Missing name\n---\n\n# Invalid\n",
        )
        .unwrap();
        skill_dir
    }

    fn basic_patterns() -> Vec<(String, String, PathBuf)> {
        vec![
            (
                "codex".to_string(),
                "Codex CLI".to_string(),
                PathBuf::from(".agents/skills"),
            ),
            (
                "claude-code".to_string(),
                "Claude Code".to_string(),
                PathBuf::from(".claude/skills"),
            ),
        ]
    }

    fn scan_for_test(root: &Path, central_dir: &Path) -> Vec<DiscoveredProject> {
        scan_root_for_projects(root, &basic_patterns(), central_dir)
    }

    fn scan_root(path: &Path, enabled: bool, exists: bool) -> ScanRoot {
        ScanRoot {
            path: path.to_string_lossy().into_owned(),
            label: "test".to_string(),
            exists,
            enabled,
        }
    }

    fn vault_manifest(root: &Path) -> Vec<(String, bool, bool, Option<Vec<u8>>)> {
        fn walk(root: &Path, current: &Path, out: &mut Vec<(String, bool, bool, Option<Vec<u8>>)>) {
            let mut entries: Vec<_> = std::fs::read_dir(current)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            entries.sort();

            for path in entries {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                let metadata = std::fs::symlink_metadata(&path).unwrap();
                let is_symlink = metadata.file_type().is_symlink();
                let is_dir = metadata.is_dir();
                let content = if metadata.is_file() {
                    Some(std::fs::read(&path).unwrap())
                } else {
                    None
                };
                out.push((rel, is_dir, is_symlink, content));
                if is_dir && !is_symlink {
                    walk(root, &path, out);
                }
            }
        }

        let mut manifest = Vec::new();
        walk(root, root, &mut manifest);
        manifest
    }

    #[tokio::test]
    async fn test_scan_root_for_projects_finds_nested_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Create a project with a .claude/skills/ subdirectory.
        let project_dir = tmp.path().join("my-project");
        let skill_dir = project_dir.join(".claude/skills/deploy-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy stuff\n---\n\n# Deploy\n",
        )
        .unwrap();

        // Build patterns: .claude/skills
        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert_eq!(projects.len(), 1, "should find 1 project");
        assert_eq!(projects[0].project_name, "my-project");
        assert_eq!(projects[0].skills.len(), 1);
        assert_eq!(projects[0].skills[0].platform_id, "claude-code");
        assert_eq!(projects[0].skills[0].name, "deploy");
    }

    #[tokio::test]
    async fn test_obsidian_exact_vault_root_discovers_vault_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let vault_dir = tmp.path().join("vault");
        std::fs::create_dir_all(vault_dir.join(".obsidian")).unwrap();
        let skill_dir = vault_dir.join(".agents/skills/research-helper");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: Research Helper\ndescription: Vault skill\n---\n\n# Research Helper\n",
        )
        .unwrap();

        let patterns = vec![(
            "codex".to_string(),
            "Codex CLI".to_string(),
            PathBuf::from(".agents/skills"),
        )];

        let projects = scan_root_for_projects(&vault_dir, &patterns, &central_dir);

        assert_eq!(projects.len(), 1, "exact vault root should be scanned");
        assert_eq!(projects[0].project_path, vault_dir.to_string_lossy());
        assert_eq!(projects[0].project_name, "vault");
        assert_eq!(projects[0].skills.len(), 1);
        assert_eq!(projects[0].skills[0].platform_id, "obsidian");
        assert_eq!(projects[0].skills[0].platform_name, "Obsidian");
        assert_eq!(projects[0].skills[0].name, "Research Helper");
    }

    #[tokio::test]
    async fn test_get_scan_roots_merges_obsidian_icloud_and_custom_roots() {
        let tmp = tempfile::TempDir::new().unwrap();
        let home = tmp.path().join("home");
        let icloud_root = home
            .join("Library")
            .join("Mobile Documents")
            .join("iCloud~md~obsidian")
            .join("Documents");
        let custom_active = tmp.path().join("custom-active-vault");
        let custom_inactive = tmp.path().join("custom-inactive-vault");
        std::fs::create_dir_all(&icloud_root).unwrap();
        std::fs::create_dir_all(&custom_active).unwrap();
        std::fs::create_dir_all(&custom_inactive).unwrap();

        let pool = setup_test_db().await;
        db::add_scan_directory(
            &pool,
            &custom_active.to_string_lossy(),
            Some("Custom Active"),
        )
        .await
        .unwrap();
        db::add_scan_directory(&pool, &custom_inactive.to_string_lossy(), None)
            .await
            .unwrap();
        db::toggle_scan_directory(&pool, &custom_inactive.to_string_lossy(), false)
            .await
            .unwrap();
        db::add_scan_directory(
            &pool,
            &icloud_root.to_string_lossy(),
            Some("Duplicate iCloud"),
        )
        .await
        .unwrap();

        let mut config = HashMap::new();
        config.insert(icloud_root.to_string_lossy().to_string(), false);
        db::set_setting(
            &pool,
            "discover_scan_roots_config",
            &serde_json::to_string(&config).unwrap(),
        )
        .await
        .unwrap();

        let roots = build_scan_roots(&pool, default_scan_roots_for_home(&home))
            .await
            .unwrap();

        let icloud_matches: Vec<_> = roots
            .iter()
            .filter(|root| root.path == icloud_root.to_string_lossy())
            .collect();
        assert_eq!(
            icloud_matches.len(),
            1,
            "default iCloud root and custom duplicate should collapse"
        );
        assert_eq!(icloud_matches[0].label, "Obsidian iCloud");
        assert!(icloud_matches[0].exists);
        assert!(
            !icloud_matches[0].enabled,
            "persisted Discover override should apply to merged default root"
        );

        let active = roots
            .iter()
            .find(|root| root.path == custom_active.to_string_lossy())
            .expect("custom active root should be merged");
        assert_eq!(active.label, "Custom Active");
        assert!(active.exists);
        assert!(active.enabled);

        let inactive = roots
            .iter()
            .find(|root| root.path == custom_inactive.to_string_lossy())
            .expect("custom inactive root should be merged");
        assert_eq!(inactive.label, "custom-inactive-vault");
        assert!(inactive.exists);
        assert!(!inactive.enabled);

        assert!(
            roots
                .iter()
                .all(|root| !root.path.ends_with(".claude/skills")),
            "built-in agent scan directory rows must not be merged into Discover roots"
        );
    }

    #[tokio::test]
    async fn test_obsidian_parent_scan_supported_locations_and_marker_requirement() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let vault_dir = tmp.path().join("marked-vault");
        std::fs::create_dir_all(vault_dir.join(".obsidian")).unwrap();
        create_skill(
            &vault_dir.join(".skills"),
            "native-skill",
            "Native Skill",
            "From .skills",
        );
        create_skill(
            &vault_dir.join(".agents/skills"),
            "agents-skill",
            "Agents Skill",
            "From .agents",
        );
        create_skill(
            &vault_dir.join(".claude/skills"),
            "claude-skill",
            "Claude Skill",
            "From .claude",
        );

        let ordinary_dir = tmp.path().join("ordinary-project");
        create_skill(
            &ordinary_dir.join(".agents/skills"),
            "ordinary-skill",
            "Ordinary Skill",
            "No vault marker",
        );

        let mut projects = scan_for_test(tmp.path(), &central_dir);
        projects.sort_by(|a, b| a.project_name.cmp(&b.project_name));

        let vault = projects
            .iter()
            .find(|project| project.project_path == vault_dir.to_string_lossy())
            .expect("marked vault should be discovered as its own project");
        assert_eq!(vault.skills.len(), 3);
        assert!(vault
            .skills
            .iter()
            .all(|skill| skill.platform_id == OBSIDIAN_PLATFORM_ID
                && skill.platform_name == OBSIDIAN_PLATFORM_NAME));
        let selected_dirs: Vec<String> = vault
            .skills
            .iter()
            .map(|skill| selected_skill_dir_name(&skill.dir_path))
            .collect();
        assert!(selected_dirs.contains(&"native-skill".to_string()));
        assert!(selected_dirs.contains(&"agents-skill".to_string()));
        assert!(selected_dirs.contains(&"claude-skill".to_string()));

        let ordinary = projects
            .iter()
            .find(|project| project.project_path == ordinary_dir.to_string_lossy())
            .expect("unmarked project should retain ordinary Discover behavior");
        assert_eq!(ordinary.skills.len(), 1);
        assert_eq!(ordinary.skills[0].platform_id, "codex");
        assert_ne!(ordinary.skills[0].platform_id, OBSIDIAN_PLATFORM_ID);
    }

    #[tokio::test]
    async fn test_obsidian_duplicate_priority_and_invalid_fallback() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();
        let vault_dir = tmp.path().join("vault");
        std::fs::create_dir_all(vault_dir.join(".obsidian")).unwrap();

        create_skill(
            &vault_dir.join(".skills"),
            "shared",
            "Shared Native",
            "native",
        );
        create_skill(
            &vault_dir.join(".agents/skills"),
            "shared",
            "Shared Agents",
            "agents",
        );
        create_skill(
            &vault_dir.join(".claude/skills"),
            "shared",
            "Shared Claude",
            "claude",
        );

        let projects = scan_for_test(&vault_dir, &central_dir);
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].skills.len(), 1);
        assert!(
            projects[0].skills[0].dir_path.contains(".skills/shared"),
            ".skills should win over lower-priority duplicates"
        );

        std::fs::remove_dir_all(vault_dir.join(".skills/shared")).unwrap();
        let projects = scan_for_test(&vault_dir, &central_dir);
        assert_eq!(projects[0].skills.len(), 1);
        assert!(
            projects[0].skills[0]
                .dir_path
                .contains(".agents/skills/shared"),
            ".agents/skills should win when .skills is absent"
        );

        std::fs::remove_dir_all(vault_dir.join(".agents/skills/shared")).unwrap();
        create_invalid_skill(&vault_dir.join(".agents/skills"), "shared");
        let projects = scan_for_test(&vault_dir, &central_dir);
        assert_eq!(projects[0].skills.len(), 1);
        assert!(
            projects[0].skills[0]
                .dir_path
                .contains(".claude/skills/shared"),
            "invalid higher-priority duplicate must fall back to valid lower-priority source"
        );
    }

    #[tokio::test]
    async fn test_obsidian_dedupe_is_scoped_by_full_vault_path() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let vault_a = tmp.path().join("team-a").join("same-name");
        let vault_b = tmp.path().join("team-b").join("same-name");
        for vault in [&vault_a, &vault_b] {
            std::fs::create_dir_all(vault.join(".obsidian")).unwrap();
            create_skill(
                &vault.join(".skills"),
                "shared-id",
                "Shared",
                "same skill id in different vaults",
            );
        }

        let projects = scan_for_test(tmp.path(), &central_dir);
        let vault_projects: Vec<_> = projects
            .iter()
            .filter(|project| project.project_name == "same-name")
            .collect();
        assert_eq!(vault_projects.len(), 2);

        let ids: std::collections::HashSet<_> = vault_projects
            .iter()
            .map(|project| project.skills[0].id.as_str())
            .collect();
        assert_eq!(
            ids.len(),
            2,
            "same basename and skill id in different vaults must remain distinct"
        );
        assert!(vault_projects
            .iter()
            .any(|project| project.project_path == vault_a.to_string_lossy()));
        assert!(vault_projects
            .iter()
            .any(|project| project.project_path == vault_b.to_string_lossy()));
    }

    #[tokio::test]
    async fn test_scan_root_for_projects_skips_dirs_without_skills() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // A project dir with no skill subdirectories.
        let project_dir = tmp.path().join("empty-project");
        std::fs::create_dir_all(project_dir.join("src")).unwrap();

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);
        assert!(
            projects.is_empty(),
            "should not find projects without skills"
        );
    }

    #[tokio::test]
    async fn test_scan_root_for_projects_handles_multiple_platforms() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let project_dir = tmp.path().join("multi-project");
        // Create skills for two platforms.
        let claude_skill = project_dir.join(".claude/skills/claude-skill");
        std::fs::create_dir_all(&claude_skill).unwrap();
        std::fs::write(
            claude_skill.join("SKILL.md"),
            "---\nname: claude-skill\ndescription: test\n---\n\n# Test\n",
        )
        .unwrap();

        let cursor_skill = project_dir.join(".cursor/skills/cursor-skill");
        std::fs::create_dir_all(&cursor_skill).unwrap();
        std::fs::write(
            cursor_skill.join("SKILL.md"),
            "---\nname: cursor-skill\ndescription: test\n---\n\n# Test\n",
        )
        .unwrap();

        let patterns = vec![
            (
                "claude-code".to_string(),
                "Claude Code".to_string(),
                PathBuf::from(".claude/skills"),
            ),
            (
                "cursor".to_string(),
                "Cursor".to_string(),
                PathBuf::from(".cursor/skills"),
            ),
        ];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].skills.len(), 2);
    }

    #[tokio::test]
    async fn test_scan_root_for_projects_detects_already_central() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Create a skill in central.
        let central_skill = central_dir.join("shared-skill");
        std::fs::create_dir_all(&central_skill).unwrap();
        std::fs::write(
            central_skill.join("SKILL.md"),
            "---\nname: shared-skill\n---\n\n# Test\n",
        )
        .unwrap();

        // Create the same skill name in a project.
        let project_dir = tmp.path().join("my-project");
        let project_skill = project_dir.join(".claude/skills/shared-skill");
        std::fs::create_dir_all(&project_skill).unwrap();
        std::fs::write(
            project_skill.join("SKILL.md"),
            "---\nname: shared-skill\n---\n\n# Test\n",
        )
        .unwrap();

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].skills.len(), 1);
        assert!(
            projects[0].skills[0].is_already_central,
            "should detect skill is already in central"
        );
    }

    #[tokio::test]
    async fn test_import_discovered_skill_to_central_copies_and_persists() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        db::init_database(&pool).await.unwrap();

        // Override central dir for testing.
        let central_dir = tmp.path().join("central");
        sqlx::query("UPDATE agents SET global_skills_dir = ? WHERE id = 'central'")
            .bind(central_dir.to_str().unwrap())
            .execute(&pool)
            .await
            .unwrap();

        std::fs::create_dir_all(&central_dir).unwrap();

        // Create a discovered skill.
        let skill_dir = tmp.path().join("project/.claude/skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\n",
        )
        .unwrap();

        // Insert discovered skill record.
        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "claude-code__project__my-skill",
            "my-skill",
            Some("A test skill"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &tmp.path().join("project").to_string_lossy(),
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        // Set HOME to tmp so import_discovered_skill_to_central finds the right dir.
        // We'll call the impl directly instead.
        let result = import_discovered_skill_to_central_impl(
            &pool,
            "claude-code__project__my-skill",
            &central_dir,
        )
        .await;

        assert!(result.is_ok(), "import should succeed: {:?}", result);

        // Verify the skill was copied to central.
        let target = central_dir.join("my-skill");
        assert!(target.exists(), "skill should be copied to central");
        assert!(
            target.join("SKILL.md").exists(),
            "SKILL.md should exist in central"
        );

        // Verify discovered skill record was removed.
        let record = db::get_discovered_skill_by_id(&pool, "claude-code__project__my-skill")
            .await
            .unwrap();
        assert!(
            record.is_none(),
            "discovered skill record should be removed"
        );
    }

    /// Implementation of import_discovered_skill_to_central that accepts a custom central_dir
    /// for testing (avoids depending on $HOME).
    async fn import_discovered_skill_to_central_impl(
        pool: &DbPool,
        discovered_skill_id: &str,
        central_dir: &Path,
    ) -> Result<ImportResult, String> {
        let skill = db::get_discovered_skill_by_id(pool, discovered_skill_id)
            .await?
            .ok_or_else(|| format!("Discovered skill '{}' not found", discovered_skill_id))?;

        let skill_dir_name = Path::new(&skill.dir_path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "Cannot extract skill directory name".to_string())?
            .to_string();

        let target_dir = central_dir.join(&skill_dir_name);

        if target_dir.exists() {
            return Err(format!(
                "A skill named '{}' already exists in central skills",
                skill_dir_name
            ));
        }

        std::fs::create_dir_all(central_dir)
            .map_err(|e| format!("Failed to create central dir: {}", e))?;

        super::super::linker::copy_dir_all(Path::new(&skill.dir_path), &target_dir)?;

        let skill_md_path = target_dir.join("SKILL.md");
        let info = super::super::scanner::parse_skill_md(&skill_md_path);

        if let Some(skill_info) = info {
            let now = Utc::now().to_rfc3339();
            let db_skill = db::Skill {
                id: skill_dir_name.clone(),
                name: skill_info.name,
                description: skill_info.description,
                file_path: skill_md_path.to_string_lossy().into_owned(),
                canonical_path: Some(target_dir.to_string_lossy().into_owned()),
                is_central: true,
                source: Some("copy".to_string()),
                content: None,
                scanned_at: now,
            };
            db::upsert_skill(pool, &db_skill).await?;
        }

        db::delete_discovered_skill(pool, discovered_skill_id).await?;

        Ok(ImportResult {
            skill_id: skill_dir_name,
            target: "central".to_string(),
        })
    }

    // ── Additional tests ──────────────────────────────────────────────────────

    /// Helper: set up an in-memory DB with initialized schema.
    async fn setup_test_db() -> DbPool {
        let pool = SqlitePool::connect(":memory:").await.unwrap();
        db::init_database(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_import_discovered_skill_to_platform_creates_symlink() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;

        // Override agent skills dir for testing.
        let agent_dir = tmp.path().join("agent-skills");
        sqlx::query("UPDATE agents SET global_skills_dir = ? WHERE id = 'claude-code'")
            .bind(agent_dir.to_str().unwrap())
            .execute(&pool)
            .await
            .unwrap();

        std::fs::create_dir_all(&agent_dir).unwrap();

        // Create a discovered skill in a project.
        let skill_dir = tmp.path().join("project/.claude/skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\n",
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "claude-code__project__my-skill",
            "my-skill",
            Some("A test skill"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &tmp.path().join("project").to_string_lossy(),
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        // Import to platform using the impl function.
        let result = import_discovered_skill_to_platform_impl(
            &pool,
            "claude-code__project__my-skill",
            "claude-code",
            &agent_dir,
        )
        .await;

        assert!(result.is_ok(), "import should succeed: {:?}", result);

        // Verify the symlink was created.
        let link_path = agent_dir.join("my-skill");
        assert!(link_path.exists(), "symlink target should exist");
        let meta = std::fs::symlink_metadata(&link_path).unwrap();
        assert!(meta.is_symlink(), "should be a symlink");

        // Verify discovered skill record is KEPT (not deleted) after platform install.
        // This enables multi-platform install — the record stays so it can be
        // installed to additional platforms.
        let record = db::get_discovered_skill_by_id(&pool, "claude-code__project__my-skill")
            .await
            .unwrap();
        assert!(
            record.is_some(),
            "discovered skill record should be kept after platform install"
        );
    }

    /// Implementation of import_discovered_skill_to_platform that accepts a custom agent_dir
    /// for testing (avoids depending on $HOME and real agent dirs).
    async fn import_discovered_skill_to_platform_impl(
        pool: &DbPool,
        discovered_skill_id: &str,
        agent_id: &str,
        agent_dir: &Path,
    ) -> Result<ImportResult, String> {
        let skill = db::get_discovered_skill_by_id(pool, discovered_skill_id)
            .await?
            .ok_or_else(|| format!("Discovered skill '{}' not found", discovered_skill_id))?;

        let skill_dir_name = Path::new(&skill.dir_path)
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or_else(|| "Cannot extract skill directory name".to_string())?
            .to_string();

        let target_path = agent_dir.join(&skill_dir_name);

        std::fs::create_dir_all(agent_dir)
            .map_err(|e| format!("Failed to create agent skills directory: {}", e))?;

        if target_path.exists() || std::fs::symlink_metadata(&target_path).is_ok() {
            return Err(format!(
                "Skill '{}' already exists in agent {}",
                skill_dir_name, agent_id
            ));
        }

        let src_path = Path::new(&skill.dir_path);
        let relative_target = super::super::linker::symlink_target_path(agent_dir, src_path);
        super::super::linker::create_symlink(&relative_target, &target_path)?;

        // Record the installation.
        let now = Utc::now().to_rfc3339();

        let skill_md_path = src_path.join("SKILL.md");
        let info = super::super::scanner::parse_skill_md(&skill_md_path);

        if let Some(skill_info) = info {
            let db_skill = db::Skill {
                id: skill_dir_name.clone(),
                name: skill_info.name,
                description: skill_info.description,
                file_path: skill_md_path.to_string_lossy().into_owned(),
                canonical_path: None,
                is_central: false,
                source: Some("symlink".to_string()),
                content: None,
                scanned_at: now.clone(),
            };
            db::upsert_skill(pool, &db_skill).await?;
        }

        let installation = db::SkillInstallation {
            skill_id: skill_dir_name.clone(),
            agent_id: agent_id.to_string(),
            installed_path: target_path.to_string_lossy().into_owned(),
            link_type: "symlink".to_string(),
            symlink_target: Some(skill.dir_path.clone()),
            created_at: now,
        };
        db::upsert_skill_installation(pool, &installation).await?;

        // NOTE: Intentionally do NOT delete the discovered skill record.
        // This allows multi-platform install (importing the same discovered
        // skill to multiple platforms in sequence).

        Ok(ImportResult {
            skill_id: skill_dir_name,
            target: agent_id.to_string(),
        })
    }

    #[tokio::test]
    async fn test_stop_project_scan_sets_cancel_flag() {
        // Before calling stop, the flag should be false.
        SCAN_CANCEL.store(false, Ordering::Relaxed);
        assert!(!SCAN_CANCEL.load(Ordering::Relaxed));

        // After calling stop, the flag should be true.
        SCAN_CANCEL.store(true, Ordering::Relaxed);
        assert!(SCAN_CANCEL.load(Ordering::Relaxed));

        // Reset for other tests.
        SCAN_CANCEL.store(false, Ordering::Relaxed);
    }

    #[tokio::test]
    async fn test_get_discovered_skills_groups_by_project() {
        let pool = setup_test_db().await;
        let now = Utc::now().to_rfc3339();

        // Insert two discovered skills in the same project.
        db::insert_discovered_skill(
            &pool,
            "claude-code__proj1__skill-a",
            "skill-a",
            Some("Skill A"),
            "/tmp/proj1/.claude/skills/skill-a/SKILL.md",
            "/tmp/proj1/.claude/skills/skill-a",
            "/tmp/proj1",
            "proj1",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        db::insert_discovered_skill(
            &pool,
            "cursor__proj1__skill-b",
            "skill-b",
            Some("Skill B"),
            "/tmp/proj1/.cursor/skills/skill-b/SKILL.md",
            "/tmp/proj1/.cursor/skills/skill-b",
            "/tmp/proj1",
            "proj1",
            "cursor",
            &now,
        )
        .await
        .unwrap();

        // Insert a skill in a different project.
        db::insert_discovered_skill(
            &pool,
            "claude-code__proj2__skill-c",
            "skill-c",
            Some("Skill C"),
            "/tmp/proj2/.claude/skills/skill-c/SKILL.md",
            "/tmp/proj2/.claude/skills/skill-c",
            "/tmp/proj2",
            "proj2",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        let rows = db::get_all_discovered_skills(&pool).await.unwrap();
        assert_eq!(rows.len(), 3, "should have 3 discovered skill rows");

        // Group by project_path.
        let mut by_project: HashMap<String, Vec<db::DiscoveredSkillRow>> = HashMap::new();
        for row in rows {
            by_project
                .entry(row.project_path.clone())
                .or_default()
                .push(row);
        }

        assert_eq!(by_project.len(), 2, "should have 2 projects");
        let proj1_skills = by_project.get("/tmp/proj1").unwrap();
        assert_eq!(proj1_skills.len(), 2, "proj1 should have 2 skills");
        let proj2_skills = by_project.get("/tmp/proj2").unwrap();
        assert_eq!(proj2_skills.len(), 1, "proj2 should have 1 skill");
    }

    #[tokio::test]
    async fn test_get_discovered_skills_recomputes_obsidian_central_status() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let central_dir = tmp.path().join("central");
        let vault_dir = tmp.path().join("vault");
        let skill_dir = create_skill(
            &vault_dir.join(".skills"),
            "cached-skill",
            "Cached Skill",
            "from cache",
        );
        std::fs::create_dir_all(central_dir.join("cached-skill")).unwrap();

        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "obsidian__fixture__cached-skill",
            "Cached Skill",
            Some("from cache"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &vault_dir.to_string_lossy(),
            "vault",
            OBSIDIAN_PLATFORM_ID,
            &now,
        )
        .await
        .unwrap();

        let projects = get_discovered_skills_impl(&pool, &central_dir)
            .await
            .unwrap();
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].skills.len(), 1);
        assert_eq!(projects[0].skills[0].platform_id, OBSIDIAN_PLATFORM_ID);
        assert_eq!(projects[0].skills[0].platform_name, OBSIDIAN_PLATFORM_NAME);
        assert!(
            projects[0].skills[0].is_already_central,
            "cached Obsidian row should recompute central status from selected dir name"
        );

        std::fs::remove_dir_all(central_dir.join("cached-skill")).unwrap();
        let projects = get_discovered_skills_impl(&pool, &central_dir)
            .await
            .unwrap();
        assert!(
            !projects[0].skills[0].is_already_central,
            "central status should not be a stale cached snapshot"
        );
    }

    #[tokio::test]
    async fn test_clear_discovered_skills_removes_all() {
        let pool = setup_test_db().await;
        let now = Utc::now().to_rfc3339();

        db::insert_discovered_skill(
            &pool,
            "id1",
            "skill-1",
            None,
            "/tmp/skill1/SKILL.md",
            "/tmp/skill1",
            "/tmp/proj1",
            "proj1",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        db::insert_discovered_skill(
            &pool,
            "id2",
            "skill-2",
            None,
            "/tmp/skill2/SKILL.md",
            "/tmp/skill2",
            "/tmp/proj1",
            "proj1",
            "cursor",
            &now,
        )
        .await
        .unwrap();

        let before = db::get_all_discovered_skills(&pool).await.unwrap();
        assert_eq!(before.len(), 2);

        db::clear_all_discovered_skills(&pool).await.unwrap();

        let after = db::get_all_discovered_skills(&pool).await.unwrap();
        assert!(after.is_empty(), "all discovered skills should be cleared");
    }

    #[tokio::test]
    async fn test_get_scan_roots_returns_defaults() {
        let pool = setup_test_db().await;

        // No persisted config yet — should return defaults.
        let roots = get_scan_roots_impl(&pool).await.unwrap();
        assert!(!roots.is_empty(), "should return default scan roots");

        // Each root should have a path and label.
        for root in &roots {
            assert!(!root.path.is_empty());
            assert!(!root.label.is_empty());
        }
    }

    #[tokio::test]
    async fn test_set_scan_root_enabled_persists_state() {
        let pool = setup_test_db().await;

        // Get defaults.
        let roots = get_scan_roots_impl(&pool).await.unwrap();
        let some_path = roots[0].path.clone();

        // Disable a root.
        set_scan_root_enabled_impl(&pool, some_path.clone(), false)
            .await
            .unwrap();

        // Verify the change is reflected.
        let updated = get_scan_roots_impl(&pool).await.unwrap();
        let changed = updated.iter().find(|r| r.path == some_path).unwrap();
        assert!(
            !changed.enabled,
            "root should be disabled after set_scan_root_enabled"
        );

        // Re-enable it.
        set_scan_root_enabled_impl(&pool, some_path.clone(), true)
            .await
            .unwrap();

        let re_updated = get_scan_roots_impl(&pool).await.unwrap();
        let re_changed = re_updated.iter().find(|r| r.path == some_path).unwrap();
        assert!(re_changed.enabled, "root should be re-enabled");
    }

    /// Implementation of get_scan_roots that takes a pool directly for testing.
    async fn get_scan_roots_impl(pool: &DbPool) -> Result<Vec<ScanRoot>, String> {
        let mut roots = default_scan_roots();

        if let Some(json) = db::get_setting(pool, "discover_scan_roots_config").await? {
            let config: HashMap<String, bool> = serde_json::from_str(&json)
                .map_err(|e| format!("Invalid scan roots config: {}", e))?;
            for root in &mut roots {
                if let Some(&enabled) = config.get(&root.path) {
                    root.enabled = enabled;
                }
            }
        }

        Ok(roots)
    }

    /// Implementation of set_scan_root_enabled that takes a pool directly for testing.
    async fn set_scan_root_enabled_impl(
        pool: &DbPool,
        path: String,
        enabled: bool,
    ) -> Result<(), String> {
        let mut config: HashMap<String, bool> =
            match db::get_setting(pool, "discover_scan_roots_config").await? {
                Some(json) => serde_json::from_str(&json)
                    .map_err(|e| format!("Invalid scan roots config: {}", e))?,
                None => HashMap::new(),
            };

        config.insert(path, enabled);

        let json = serde_json::to_string(&config)
            .map_err(|e| format!("Failed to serialize scan roots config: {}", e))?;
        db::set_setting(pool, "discover_scan_roots_config", &json).await
    }

    #[tokio::test]
    async fn test_scan_cancellation_stops_early() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Create multiple project dirs with skills.
        for i in 0..5 {
            let project_dir = tmp.path().join(format!("project-{}", i));
            let skill_dir = project_dir.join(".claude/skills/deploy-skill");
            std::fs::create_dir_all(&skill_dir).unwrap();
            std::fs::write(
                skill_dir.join("SKILL.md"),
                format!(
                    "---\nname: deploy-{}\ndescription: Deploy stuff\n---\n\n# Deploy {}\n",
                    i, i
                ),
            )
            .unwrap();
        }

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        // Set cancel flag before scanning.
        SCAN_CANCEL.store(true, Ordering::Relaxed);

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);
        assert!(
            projects.is_empty(),
            "should find no projects when cancel flag is set"
        );

        // Reset for other tests.
        SCAN_CANCEL.store(false, Ordering::Relaxed);
    }

    #[tokio::test]
    async fn test_discovered_skill_insert_and_get_by_id() {
        let pool = setup_test_db().await;
        let now = Utc::now().to_rfc3339();

        db::insert_discovered_skill(
            &pool,
            "test-id-1",
            "test-skill",
            Some("A description"),
            "/tmp/project/.claude/skills/test-skill/SKILL.md",
            "/tmp/project/.claude/skills/test-skill",
            "/tmp/project",
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        let found = db::get_discovered_skill_by_id(&pool, "test-id-1")
            .await
            .unwrap();
        assert!(found.is_some());
        let row = found.unwrap();
        assert_eq!(row.name, "test-skill");
        assert_eq!(row.platform_id, "claude-code");
        assert_eq!(row.project_name, "project");

        let not_found = db::get_discovered_skill_by_id(&pool, "nonexistent")
            .await
            .unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_insert_discovered_skill_upserts_metadata() {
        let pool = setup_test_db().await;
        let now = Utc::now().to_rfc3339();

        db::insert_discovered_skill(
            &pool,
            "dup-id",
            "dup-skill",
            None,
            "/tmp/dup/SKILL.md",
            "/tmp/dup",
            "/tmp/proj",
            "proj",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        db::insert_discovered_skill(
            &pool,
            "dup-id",
            "dup-skill-updated",
            Some("updated description"),
            "/tmp/dup-new/SKILL.md",
            "/tmp/dup-new",
            "/tmp/proj",
            "proj",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        let rows = db::get_all_discovered_skills(&pool).await.unwrap();
        assert_eq!(rows.len(), 1, "should still have only 1 row");
        assert_eq!(rows[0].name, "dup-skill-updated");
        assert_eq!(rows[0].description.as_deref(), Some("updated description"));
        assert_eq!(rows[0].dir_path, "/tmp/dup-new");
    }

    #[tokio::test]
    async fn test_delete_discovered_skill() {
        let pool = setup_test_db().await;
        let now = Utc::now().to_rfc3339();

        db::insert_discovered_skill(
            &pool,
            "to-delete",
            "delete-me",
            None,
            "/tmp/del/SKILL.md",
            "/tmp/del",
            "/tmp/proj",
            "proj",
            "cursor",
            &now,
        )
        .await
        .unwrap();

        let found = db::get_discovered_skill_by_id(&pool, "to-delete")
            .await
            .unwrap();
        assert!(found.is_some());

        db::delete_discovered_skill(&pool, "to-delete")
            .await
            .unwrap();

        let gone = db::get_discovered_skill_by_id(&pool, "to-delete")
            .await
            .unwrap();
        assert!(gone.is_none());
    }

    #[tokio::test]
    async fn test_import_to_central_refuses_duplicate() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Create a skill already in central.
        let existing = central_dir.join("existing-skill");
        std::fs::create_dir_all(&existing).unwrap();
        std::fs::write(
            existing.join("SKILL.md"),
            "---\nname: existing-skill\n---\n\n# Test\n",
        )
        .unwrap();

        // Also create the same skill in a project (discovered).
        let project_skill = tmp.path().join("project/.claude/skills/existing-skill");
        std::fs::create_dir_all(&project_skill).unwrap();
        std::fs::write(
            project_skill.join("SKILL.md"),
            "---\nname: existing-skill\n---\n\n# Test\n",
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "claude-code__project__existing-skill",
            "existing-skill",
            None,
            &project_skill.join("SKILL.md").to_string_lossy(),
            &project_skill.to_string_lossy(),
            &tmp.path().join("project").to_string_lossy(),
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        let result = import_discovered_skill_to_central_impl(
            &pool,
            "claude-code__project__existing-skill",
            &central_dir,
        )
        .await;

        assert!(
            result.is_err(),
            "should refuse to import when skill already exists in central"
        );
    }

    #[tokio::test]
    async fn test_import_to_platform_refuses_existing_installation() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let agent_dir = tmp.path().join("agent-skills");
        std::fs::create_dir_all(&agent_dir).unwrap();

        // Create an existing skill in agent dir.
        let existing = agent_dir.join("existing-skill");
        std::fs::create_dir_all(&existing).unwrap();

        // Also create a discovered skill with the same name.
        let project_skill = tmp.path().join("project/.claude/skills/existing-skill");
        std::fs::create_dir_all(&project_skill).unwrap();
        std::fs::write(
            project_skill.join("SKILL.md"),
            "---\nname: existing-skill\n---\n\n# Test\n",
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "claude-code__project__existing-skill",
            "existing-skill",
            None,
            &project_skill.join("SKILL.md").to_string_lossy(),
            &project_skill.to_string_lossy(),
            &tmp.path().join("project").to_string_lossy(),
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        let result = import_discovered_skill_to_platform_impl(
            &pool,
            "claude-code__project__existing-skill",
            "claude-code",
            &agent_dir,
        )
        .await;

        assert!(
            result.is_err(),
            "should refuse to import when skill already exists in agent dir"
        );
    }

    #[tokio::test]
    async fn test_platform_skill_patterns_includes_all_non_central_agents() {
        let pool = setup_test_db().await;
        let patterns = platform_skill_patterns(&pool);

        // Should have entries for all non-central agents.
        let central_agents: Vec<_> = db::builtin_agents()
            .iter()
            .filter(|a| a.id != "central")
            .map(|a| a.id.clone())
            .collect();

        for agent_id in &central_agents {
            assert!(
                patterns.iter().any(|(id, _, _)| id == agent_id),
                "agent '{}' should appear in platform skill patterns",
                agent_id
            );
        }
    }

    #[tokio::test]
    async fn test_discovered_project_count() {
        let pool = setup_test_db().await;
        let now = Utc::now().to_rfc3339();

        // Insert skills across 3 different projects.
        for i in 0..3 {
            db::insert_discovered_skill(
                &pool,
                &format!("skill-{}", i),
                &format!("skill {}", i),
                None,
                &format!("/tmp/proj{}/SKILL.md", i),
                &format!("/tmp/proj{}", i),
                &format!("/tmp/proj{}", i),
                &format!("proj{}", i),
                "claude-code",
                &now,
            )
            .await
            .unwrap();
        }

        let count = db::get_discovered_project_count(&pool).await.unwrap();
        assert_eq!(count, 3, "should have 3 distinct projects");
    }

    // ── Recursive scan tests ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_recursive_scan_finds_deeply_nested_project() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Create a project nested 3 levels deep: root/org/team/my-project/.claude/skills/...
        let project_dir = tmp.path().join("org").join("team").join("my-project");
        let skill_dir = project_dir.join(".claude/skills/deploy-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy stuff\n---\n\n# Deploy\n",
        )
        .unwrap();

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert_eq!(projects.len(), 1, "should find 1 project at depth 3");
        assert_eq!(projects[0].project_name, "my-project");
        assert_eq!(projects[0].skills.len(), 1);
        assert_eq!(projects[0].skills[0].platform_id, "claude-code");
        assert_eq!(projects[0].skills[0].name, "deploy");
        // project_path should be the directory containing the platform dir
        assert!(
            projects[0].project_path.contains("my-project"),
            "project_path should be the project dir, got: {}",
            projects[0].project_path
        );
    }

    #[tokio::test]
    async fn test_recursive_scan_skips_hidden_dirs_at_root() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // A hidden directory at root level should be skipped (not traversed).
        let hidden_project = tmp.path().join(".hidden-org").join("my-project");
        let skill_dir = hidden_project.join(".claude/skills/deploy-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deploy\ndescription: Deploy stuff\n---\n\n# Deploy\n",
        )
        .unwrap();

        // A visible directory should be traversed.
        let visible_project = tmp.path().join("visible-org").join("my-project");
        let visible_skill_dir = visible_project.join(".claude/skills/visible-skill");
        std::fs::create_dir_all(&visible_skill_dir).unwrap();
        std::fs::write(
            visible_skill_dir.join("SKILL.md"),
            "---\nname: visible-skill\ndescription: Visible\n---\n\n# Visible\n",
        )
        .unwrap();

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        // Should only find the project in the visible directory.
        assert_eq!(projects.len(), 1, "should only find the visible project");
        assert_eq!(projects[0].skills[0].name, "visible-skill");
    }

    #[tokio::test]
    async fn test_recursive_scan_skips_node_modules_and_git() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // node_modules with a skill inside should NOT be found.
        let nm_project = tmp.path().join("node_modules").join("some-pkg");
        let nm_skill = nm_project.join(".claude/skills/hidden-skill");
        std::fs::create_dir_all(&nm_skill).unwrap();
        std::fs::write(
            nm_skill.join("SKILL.md"),
            "---\nname: hidden-skill\n---\n\n# Hidden\n",
        )
        .unwrap();

        // .git with a skill inside should NOT be found.
        let git_project = tmp.path().join(".git").join("subdir");
        let git_skill = git_project.join(".claude/skills/git-skill");
        std::fs::create_dir_all(&git_skill).unwrap();
        std::fs::write(
            git_skill.join("SKILL.md"),
            "---\nname: git-skill\n---\n\n# Git\n",
        )
        .unwrap();

        // A normal project should be found.
        let project_dir = tmp.path().join("my-project");
        let skill_dir = project_dir.join(".claude/skills/good-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: good-skill\ndescription: Good\n---\n\n# Good\n",
        )
        .unwrap();

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert_eq!(projects.len(), 1, "should only find the good project");
        assert_eq!(projects[0].skills[0].name, "good-skill");
    }

    #[tokio::test]
    async fn test_recursive_scan_finds_multiple_projects_at_different_depths() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Project at depth 1 (immediate child).
        let project1 = tmp.path().join("project-1");
        let skill1 = project1.join(".claude/skills/skill-1");
        std::fs::create_dir_all(&skill1).unwrap();
        std::fs::write(
            skill1.join("SKILL.md"),
            "---\nname: skill-1\ndescription: First\n---\n\n# First\n",
        )
        .unwrap();

        // Project at depth 3 (nested under org/team).
        let project2 = tmp.path().join("org").join("team").join("project-2");
        let skill2 = project2.join(".factory/skills/skill-2");
        std::fs::create_dir_all(&skill2).unwrap();
        std::fs::write(
            skill2.join("SKILL.md"),
            "---\nname: skill-2\ndescription: Second\n---\n\n# Second\n",
        )
        .unwrap();

        let patterns = vec![
            (
                "claude-code".to_string(),
                "Claude Code".to_string(),
                PathBuf::from(".claude/skills"),
            ),
            (
                "factory-droid".to_string(),
                "Factory Droid".to_string(),
                PathBuf::from(".factory/skills"),
            ),
        ];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert_eq!(
            projects.len(),
            2,
            "should find 2 projects at different depths"
        );
        let names: Vec<&str> = projects.iter().map(|p| p.project_name.as_str()).collect();
        assert!(names.contains(&"project-1"), "should find project-1");
        assert!(names.contains(&"project-2"), "should find project-2");
    }

    #[tokio::test]
    async fn test_recursive_scan_respects_max_depth() {
        let tmp = tempfile::TempDir::new().unwrap();
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        // Create a project deeper than MAX_SCAN_DEPTH.
        // MAX_SCAN_DEPTH = 8, so depth 10 should not be reached.
        let mut deep_path = tmp.path().to_path_buf();
        for i in 0..10 {
            deep_path = deep_path.join(format!("level-{}", i));
        }
        let skill_dir = deep_path.join(".claude/skills/deep-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: deep-skill\ndescription: Too deep\n---\n\n# Deep\n",
        )
        .unwrap();

        let patterns = vec![(
            "claude-code".to_string(),
            "Claude Code".to_string(),
            PathBuf::from(".claude/skills"),
        )];

        let projects = scan_root_for_projects(tmp.path(), &patterns, &central_dir);

        assert!(
            projects.is_empty(),
            "should not find projects beyond MAX_SCAN_DEPTH"
        );
    }

    #[tokio::test]
    async fn test_should_skip_dir_rules() {
        // Always-skipped directories.
        assert!(should_skip_dir("node_modules", 0));
        assert!(should_skip_dir("node_modules", 5));
        assert!(should_skip_dir("target", 0));
        assert!(should_skip_dir("target", 3));
        assert!(should_skip_dir(".git", 0));
        assert!(should_skip_dir(".git", 5));
        assert!(should_skip_dir("build", 0));
        assert!(should_skip_dir("dist", 0));
        assert!(should_skip_dir("__pycache__", 0));
        assert!(should_skip_dir(".cache", 0));

        // Hidden dirs at root level (depth 0) should be skipped.
        assert!(should_skip_dir(".config", 0));
        assert!(should_skip_dir(".local", 0));
        assert!(should_skip_dir(".hidden-project", 0));

        // Hidden dirs at deeper levels should NOT be skipped
        // (they might contain platform patterns like .claude).
        assert!(!should_skip_dir(".claude", 1));
        assert!(!should_skip_dir(".hidden-project", 2));

        // Normal directories should never be skipped.
        assert!(!should_skip_dir("my-project", 0));
        assert!(!should_skip_dir("src", 0));
        assert!(!should_skip_dir("Documents", 0));
        assert!(!should_skip_dir("projects", 1));
    }

    // ── Cache reconciliation tests ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_cache_reconciliation_removes_stale_skills() {
        let pool = setup_test_db().await;
        let tmp = tempfile::TempDir::new().unwrap();
        let now = Utc::now().to_rfc3339();

        // Create a real skill on disk under the scan root.
        let project_dir = tmp.path().join("project");
        let skill_dir = project_dir.join(".claude/skills/real-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: real-skill\ndescription: Exists\n---\n\n# Real\n",
        )
        .unwrap();

        // Insert a discovered skill for a path that EXISTS.
        db::insert_discovered_skill(
            &pool,
            "claude-code__project__real-skill",
            "real-skill",
            Some("Exists"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &project_dir.to_string_lossy(),
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        // Insert a discovered skill whose project_path is under the scan root
        // but whose dir_path no longer exists on disk.
        let stale_project_dir = tmp.path().join("stale-project");
        let stale_skill_dir = stale_project_dir.join(".claude/skills/stale-skill");
        // NOTE: We do NOT create the stale directory on disk.
        db::insert_discovered_skill(
            &pool,
            "claude-code__stale-project__stale-skill",
            "stale-skill",
            Some("Deleted"),
            &stale_skill_dir.join("SKILL.md").to_string_lossy(),
            &stale_skill_dir.to_string_lossy(),
            &stale_project_dir.to_string_lossy(),
            "stale-project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        // Simulate a scan: the real skill was found, the stale one was not.
        let scan_root = ScanRoot {
            path: tmp.path().to_string_lossy().into_owned(),
            label: "test".to_string(),
            exists: true,
            enabled: true,
        };

        let found_ids = vec!["claude-code__project__real-skill".to_string()];

        reconcile_discovered_skills(&pool, &[&scan_root], &found_ids)
            .await
            .unwrap();

        // The real skill should still be in the DB.
        let real = db::get_discovered_skill_by_id(&pool, "claude-code__project__real-skill")
            .await
            .unwrap();
        assert!(real.is_some(), "real skill should remain in DB");

        // The stale skill should be removed from the DB.
        let stale =
            db::get_discovered_skill_by_id(&pool, "claude-code__stale-project__stale-skill")
                .await
                .unwrap();
        assert!(stale.is_none(), "stale skill should be removed from DB");
    }

    #[tokio::test]
    async fn test_cache_reconciliation_only_affects_scanned_scope() {
        let pool = setup_test_db().await;
        let tmp = tempfile::TempDir::new().unwrap();
        let now = Utc::now().to_rfc3339();

        // Insert a stale skill whose project_path is NOT under the scanned root.
        db::insert_discovered_skill(
            &pool,
            "claude-code__other__stale-skill",
            "stale-skill",
            Some("Outside scope"),
            "/other/location/.claude/skills/stale-skill/SKILL.md",
            "/other/location/.claude/skills/stale-skill",
            "/other/location",
            "other",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        // Scan a different root — the stale skill is outside the scope.
        let scan_root = ScanRoot {
            path: tmp.path().to_string_lossy().into_owned(),
            label: "test".to_string(),
            exists: true,
            enabled: true,
        };

        let found_ids: Vec<String> = vec![];

        reconcile_discovered_skills(&pool, &[&scan_root], &found_ids)
            .await
            .unwrap();

        // The stale skill should still be in the DB (outside scanned scope).
        let outside = db::get_discovered_skill_by_id(&pool, "claude-code__other__stale-skill")
            .await
            .unwrap();
        assert!(
            outside.is_some(),
            "stale skill outside scan scope should remain in DB"
        );
    }

    #[tokio::test]
    async fn test_multi_platform_install_keeps_discovered_record() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;

        // Set up two agent dirs.
        let agent_dir_a = tmp.path().join("agent-a-skills");
        let agent_dir_b = tmp.path().join("agent-b-skills");
        std::fs::create_dir_all(&agent_dir_a).unwrap();
        std::fs::create_dir_all(&agent_dir_b).unwrap();

        // Create a discovered skill.
        let skill_dir = tmp.path().join("project/.claude/skills/my-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\n\n# My Skill\n",
        )
        .unwrap();

        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "claude-code__project__my-skill",
            "my-skill",
            Some("A test skill"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &tmp.path().join("project").to_string_lossy(),
            "project",
            "claude-code",
            &now,
        )
        .await
        .unwrap();

        // Import to first platform.
        let result_a = import_discovered_skill_to_platform_impl(
            &pool,
            "claude-code__project__my-skill",
            "agent-a",
            &agent_dir_a,
        )
        .await;
        assert!(result_a.is_ok(), "first import should succeed");

        // Import to second platform — this should also succeed because
        // the discovered record is NOT deleted after platform install.
        let result_b = import_discovered_skill_to_platform_impl(
            &pool,
            "claude-code__project__my-skill",
            "agent-b",
            &agent_dir_b,
        )
        .await;
        assert!(result_b.is_ok(), "second import should succeed");

        // Both symlinks should exist.
        assert!(agent_dir_a.join("my-skill").exists());
        assert!(agent_dir_b.join("my-skill").exists());

        // Discovered record should still exist.
        let record = db::get_discovered_skill_by_id(&pool, "claude-code__project__my-skill")
            .await
            .unwrap();
        assert!(
            record.is_some(),
            "discovered record should be kept after platform installs"
        );
    }

    #[tokio::test]
    async fn test_obsidian_start_scan_persists_updates_and_reconciles_marker_removal() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let vault_dir = tmp.path().join("vault");
        std::fs::create_dir_all(vault_dir.join(".obsidian")).unwrap();
        let lower_priority = create_skill(
            &vault_dir.join(".agents/skills"),
            "changing-skill",
            "Changing Skill",
            "old description",
        );

        let roots = vec![scan_root(tmp.path(), true, true)];
        let first = start_project_scan_impl(&pool, roots.clone(), &central_dir, |_| {})
            .await
            .unwrap();
        assert_eq!(first.total_projects, 1);
        assert_eq!(first.total_skills, 1);
        let discovered_id = first.projects[0].skills[0].id.clone();

        let row = db::get_discovered_skill_by_id(&pool, &discovered_id)
            .await
            .unwrap()
            .expect("first scan should persist Obsidian row");
        assert_eq!(row.description.as_deref(), Some("old description"));
        assert_eq!(row.platform_id, OBSIDIAN_PLATFORM_ID);
        assert_eq!(row.dir_path, lower_priority.to_string_lossy());

        let higher_priority = create_skill(
            &vault_dir.join(".skills"),
            "changing-skill",
            "Changing Skill",
            "new description",
        );
        let second = start_project_scan_impl(&pool, roots.clone(), &central_dir, |_| {})
            .await
            .unwrap();
        assert_eq!(second.total_projects, 1);
        assert_eq!(second.total_skills, 1);
        assert_eq!(
            second.projects[0].skills[0].id, discovered_id,
            "dedupe identity should stay stable when selected source changes"
        );

        let updated = db::get_discovered_skill_by_id(&pool, &discovered_id)
            .await
            .unwrap()
            .expect("row should still exist after selected source update");
        assert_eq!(updated.description.as_deref(), Some("new description"));
        assert_eq!(updated.dir_path, higher_priority.to_string_lossy());

        std::fs::remove_dir_all(vault_dir.join(".obsidian")).unwrap();
        let third = start_project_scan_impl(&pool, roots, &central_dir, |_| {})
            .await
            .unwrap();
        assert_eq!(third.total_projects, 1);
        assert_eq!(
            third.projects[0].skills[0].platform_id, "codex",
            "after marker removal the remaining .agents/skills directory is ordinary Discover data"
        );
        let stale = db::get_discovered_skill_by_id(&pool, &discovered_id)
            .await
            .unwrap();
        assert!(
            stale.is_none(),
            "stale platform_id=obsidian row should be removed after marker removal"
        );
    }

    #[tokio::test]
    async fn test_obsidian_scan_is_read_only_and_does_not_create_managed_state() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let vault_dir = tmp.path().join("readonly-vault");
        std::fs::create_dir_all(vault_dir.join(".obsidian")).unwrap();
        create_skill(
            &vault_dir.join(".claude/skills"),
            "readonly-skill",
            "Readonly Skill",
            "must not mutate vault",
        );
        let before_manifest = vault_manifest(&vault_dir);

        let skills_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skills")
            .fetch_one(&pool)
            .await
            .unwrap();
        let installs_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skill_installations")
            .fetch_one(&pool)
            .await
            .unwrap();
        let agents_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agents")
            .fetch_one(&pool)
            .await
            .unwrap();

        let result = start_project_scan_impl(
            &pool,
            vec![scan_root(&vault_dir, true, true)],
            &central_dir,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(result.total_projects, 1);
        assert_eq!(result.total_skills, 1);

        let after_manifest = vault_manifest(&vault_dir);
        assert_eq!(
            before_manifest, after_manifest,
            "Discover scan must not create, remove, rewrite, or relink vault files"
        );

        let skills_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skills")
            .fetch_one(&pool)
            .await
            .unwrap();
        let installs_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skill_installations")
            .fetch_one(&pool)
            .await
            .unwrap();
        let agents_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM agents")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(skills_after, skills_before);
        assert_eq!(installs_after, installs_before);
        assert_eq!(agents_after, agents_before);
        assert!(
            db::get_agent_by_id(&pool, OBSIDIAN_PLATFORM_ID)
                .await
                .unwrap()
                .is_none(),
            "Obsidian must not be seeded as an agent"
        );
    }

    #[tokio::test]
    async fn test_obsidian_disabled_or_missing_roots_do_not_scan_or_reconcile() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let disabled_vault = tmp.path().join("disabled-vault");
        std::fs::create_dir_all(disabled_vault.join(".obsidian")).unwrap();
        let skill_dir = create_skill(
            &disabled_vault.join(".skills"),
            "disabled-skill",
            "Disabled Skill",
            "not scanned",
        );
        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "preexisting-disabled",
            "Disabled Skill",
            Some("cached"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &disabled_vault.to_string_lossy(),
            "disabled-vault",
            OBSIDIAN_PLATFORM_ID,
            &now,
        )
        .await
        .unwrap();
        std::fs::remove_dir_all(&skill_dir).unwrap();

        let result = start_project_scan_impl(
            &pool,
            vec![
                scan_root(&disabled_vault, false, true),
                scan_root(&tmp.path().join("missing"), true, false),
            ],
            &central_dir,
            |_| {},
        )
        .await
        .unwrap();

        assert_eq!(result.total_projects, 0);
        assert_eq!(result.total_skills, 0);
        assert!(
            db::get_discovered_skill_by_id(&pool, "preexisting-disabled")
                .await
                .unwrap()
                .is_some(),
            "disabled roots must not scan or reconcile cached rows"
        );
    }

    #[tokio::test]
    async fn test_obsidian_import_platform_target_is_rejected() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let skill_dir = create_skill(
            &tmp.path().join("vault/.skills"),
            "portable-skill",
            "Portable Skill",
            "source only",
        );
        let now = Utc::now().to_rfc3339();
        db::insert_discovered_skill(
            &pool,
            "obsidian__fixture__portable-skill",
            "Portable Skill",
            Some("source only"),
            &skill_dir.join("SKILL.md").to_string_lossy(),
            &skill_dir.to_string_lossy(),
            &tmp.path().join("vault").to_string_lossy(),
            "vault",
            OBSIDIAN_PLATFORM_ID,
            &now,
        )
        .await
        .unwrap();

        let result = import_discovered_skill_to_platform_from_pool(
            &pool,
            "obsidian__fixture__portable-skill",
            OBSIDIAN_PLATFORM_ID,
        )
        .await;
        assert!(result.is_err(), "Obsidian is not an install target");

        let skills: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skills")
            .fetch_one(&pool)
            .await
            .unwrap();
        let installs: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM skill_installations")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(skills.0, 0);
        assert_eq!(installs.0, 0);
    }

    #[tokio::test]
    async fn test_obsidian_streaming_events_use_deduped_totals() {
        let tmp = tempfile::TempDir::new().unwrap();
        let pool = setup_test_db().await;
        let central_dir = tmp.path().join("central");
        std::fs::create_dir_all(&central_dir).unwrap();

        let vault_dir = tmp.path().join("vault");
        std::fs::create_dir_all(vault_dir.join(".obsidian")).unwrap();
        create_invalid_skill(&vault_dir.join(".skills"), "deduped");
        create_skill(
            &vault_dir.join(".agents/skills"),
            "deduped",
            "Deduped Skill",
            "valid fallback",
        );
        create_skill(
            &vault_dir.join(".claude/skills"),
            "deduped",
            "Deduped Skill",
            "lower duplicate",
        );

        let empty_vault = tmp.path().join("empty-vault");
        std::fs::create_dir_all(empty_vault.join(".obsidian")).unwrap();

        let mut events = Vec::new();
        let result = start_project_scan_impl(
            &pool,
            vec![
                scan_root(tmp.path(), true, true),
                scan_root(&vault_dir, true, true),
            ],
            &central_dir,
            |event| events.push(event),
        )
        .await
        .unwrap();

        assert_eq!(result.total_projects, 1);
        assert_eq!(result.total_skills, 1);
        let found_count = events
            .iter()
            .filter(|event| matches!(event, DiscoverEvent::Found(_)))
            .count();
        assert_eq!(
            found_count, 1,
            "overlapping roots should emit one found event"
        );

        let complete = events
            .iter()
            .find_map(|event| match event {
                DiscoverEvent::Complete(payload) => Some(payload),
                _ => None,
            })
            .expect("complete event should be emitted");
        assert_eq!(complete.total_projects, 1);
        assert_eq!(complete.total_skills, 1);

        let last_progress = events.iter().rev().find_map(|event| match event {
            DiscoverEvent::Progress(payload) => Some(payload),
            _ => None,
        });
        let last_progress = last_progress.expect("progress event should be emitted");
        assert_eq!(last_progress.projects_found, 1);
        assert_eq!(last_progress.skills_found, 1);
    }
}
