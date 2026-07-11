use crate::core::state::AppState;
use crate::persistence::db::VoxDb;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Serialize, Clone)]
pub struct ProfileEntry {
    pub key: String,
    pub category: String,
    pub value: String,
    pub updated_at: i64,
}

#[tauri::command]
pub async fn get_personal_profile(
    _state: State<'_, std::sync::Arc<AppState>>,
) -> Result<Vec<ProfileEntry>, String> {
    let db_path = crate::utils::paths::get().db.clone();
    let conn = VoxDb::open_readonly(&db_path)
        .await
        .map_err(|e| format!("DB open failed: {}", e))?;

    let mut rows = conn
        .query(
            "SELECT key, category, value, updated_at FROM personal_memory ORDER BY category, key",
            (),
        )
        .await
        .map_err(|e| e.to_string())?;

    let mut profile = Vec::new();
    while let Some(row) = rows.next().await.map_err(|e| e.to_string())? {
        profile.push(ProfileEntry {
            key: row.get(0).map_err(|e| e.to_string())?,
            category: row.get(1).map_err(|e| e.to_string())?,
            value: row.get(2).map_err(|e| e.to_string())?,
            updated_at: row.get(3).map_err(|e| e.to_string())?,
        });
    }

    Ok(profile)
}
