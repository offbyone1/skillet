mod health;
mod launch;
mod launcher;
mod mcp;
mod profiles;
mod skills;
mod util;

use health::Finding;
use mcp::McpServer;
use profiles::Loadout;
use skills::Skill;

#[tauri::command]
fn scan_skills() -> Vec<Skill> {
    skills::scan_all()
}

#[tauri::command]
fn list_mcp() -> Vec<McpServer> {
    mcp::list()
}

#[tauri::command]
fn health_check() -> Vec<Finding> {
    let s = skills::scan_all();
    let m = mcp::list();
    health::analyze(&s, &m)
}

#[tauri::command]
fn claude_version() -> String {
    util::claude_version()
}

#[tauri::command]
fn list_profiles() -> Vec<Loadout> {
    profiles::list()
}

#[tauri::command]
fn save_profile(profile: Loadout) -> Result<(), String> {
    profiles::save(profile)
}

#[tauri::command]
fn delete_profile(id: String) -> Result<(), String> {
    profiles::delete(&id)
}

/// Trust-relevant elements in `workdir` (for the launch trust-gate, §4.2).
#[tauri::command]
fn preflight_workdir(workdir: String) -> Vec<String> {
    launch::preflight_scan(&workdir)
}

/// Launch the agent for the profile `id`, optionally overriding its workdir.
#[tauri::command]
fn launch_agent(id: String, workdir_override: Option<String>) -> Result<String, String> {
    let profile = profiles::list()
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| format!("Profil nicht gefunden: {id}"))?;
    launch::run(&profile, workdir_override)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    launch::cleanup_old();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            scan_skills,
            list_mcp,
            health_check,
            claude_version,
            list_profiles,
            save_profile,
            delete_profile,
            preflight_workdir,
            launch_agent
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
