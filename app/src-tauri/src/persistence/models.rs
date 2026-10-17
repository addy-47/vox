use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use turso::Connection;

use crate::core::error::PersistenceError;

pub type Result<T> = std::result::Result<T, PersistenceError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRecord {
    pub id: String,
    pub model_type: String,
    pub category: String,
    pub provider: Option<String>,
    pub parameters: serde_json::Value,
    pub is_favorite: bool,
    pub last_active_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// Retrieves a model record by ID, if present.
pub async fn get_model(conn: &Connection, id: &str) -> Result<Option<ModelRecord>> {
    let clean_id = id.trim();
    let mut rows = conn
        .query(
            "SELECT id, model_type, category, provider, parameters, is_favorite, last_active_at, created_at, updated_at \
             FROM models WHERE id = ?1 LIMIT 1",
            (clean_id.to_string(),),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let raw_params: String = row.get(4).unwrap_or_else(|_| "{}".to_string());
        let params: serde_json::Value =
            serde_json::from_str(&raw_params).unwrap_or_else(|_| serde_json::json!({}));
        let is_fav_int: i64 = row.get(5).unwrap_or(0);

        Ok(Some(ModelRecord {
            id: row.get(0)?,
            model_type: row.get(1)?,
            category: row.get(2)?,
            provider: row.get(3).ok(),
            parameters: params,
            is_favorite: is_fav_int != 0,
            last_active_at: row.get(6).ok(),
            created_at: row.get(7)?,
            updated_at: row.get(8)?,
        }))
    } else {
        Ok(None)
    }
}

/// Fetches stored parameter JSON for a model, or empty object if absent.
pub async fn get_model_parameters(conn: &Connection, id: &str) -> Result<serde_json::Value> {
    if let Some(record) = get_model(conn, id).await? {
        Ok(record.parameters)
    } else {
        Ok(serde_json::json!({}))
    }
}

/// Returns all non-empty per-model parameter overrides indexed by model ID.
pub async fn get_all_model_parameters(
    conn: &Connection,
) -> Result<std::collections::HashMap<String, serde_json::Value>> {
    let mut rows = conn
        .query(
            "SELECT id, parameters FROM models WHERE parameters != '{}' AND parameters != ''",
            (),
        )
        .await?;

    let mut map = std::collections::HashMap::new();
    while let Some(row) = rows.next().await? {
        if let (Ok(id), Ok(raw_params)) = (row.get::<String>(0), row.get::<String>(1)) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw_params) {
                if val.as_object().is_some_and(|o| !o.is_empty()) {
                    map.insert(id, val);
                }
            }
        }
    }
    Ok(map)
}

/// Records a model as recently activated, creating the row if it does not exist.
pub async fn upsert_model_active(
    conn: &Connection,
    id: &str,
    model_type: &str,
    category: &str,
    provider: Option<&str>,
) -> Result<()> {
    let clean_id = id.trim();
    let now = now_ms();
    let prov = provider.map(|s| s.to_string());
    conn.execute(
        "INSERT INTO models (id, model_type, category, provider, parameters, is_favorite, last_active_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, '{}', 0, ?5, ?5, ?5) \
         ON CONFLICT(id) DO UPDATE SET \
            last_active_at = ?5, \
            updated_at = ?5, \
            category = ?3, \
            provider = COALESCE(?4, models.provider);",
        (clean_id.to_string(), model_type.to_string(), category.to_string(), prov, now),
    )
    .await?;
    Ok(())
}

/// Persists parameter customization overrides for a specific model without affecting other models.
pub async fn update_model_parameters(
    conn: &Connection,
    id: &str,
    params: &serde_json::Value,
) -> Result<()> {
    let clean_id = id.trim();
    let now = now_ms();
    let existing = get_model(conn, clean_id).await?;
    let (model_type, category, provider, merged_params) = if let Some(rec) = existing {
        let mut base = rec.parameters;
        if let (Some(base_obj), Some(patch_obj)) = (base.as_object_mut(), params.as_object()) {
            for (k, v) in patch_obj {
                if v.is_null() {
                    base_obj.remove(k);
                } else {
                    base_obj.insert(k.clone(), v.clone());
                }
            }
        } else {
            base = params.clone();
        }
        (rec.model_type, rec.category, rec.provider, base)
    } else {
        (
            "custom".to_string(),
            "custom".to_string(),
            None,
            params.clone(),
        )
    };

    let params_str = serde_json::to_string(&merged_params).unwrap_or_else(|_| "{}".to_string());
    conn.execute(
        "INSERT INTO models (id, model_type, category, provider, parameters, is_favorite, last_active_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?6, ?6) \
         ON CONFLICT(id) DO UPDATE SET \
            parameters = ?5, \
            updated_at = ?6;",
        (clean_id.to_string(), model_type, category, provider, params_str, now),
    )
    .await?;
    Ok(())
}

/// Atomically toggles favorite status for a model in the `models` table. Returns new favorite boolean.
pub async fn toggle_model_favorite(conn: &Connection, id: &str) -> Result<bool> {
    let clean_id = id.trim();
    let now = now_ms();
    let existing = get_model(conn, clean_id).await?;
    let (model_type, category, provider) = if let Some(rec) = existing {
        (rec.model_type, rec.category, rec.provider)
    } else {
        ("custom".to_string(), "custom".to_string(), None)
    };

    conn.execute(
        "INSERT INTO models (id, model_type, category, provider, parameters, is_favorite, last_active_at, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, '{}', 0, NULL, ?5, ?5) \
         ON CONFLICT(id) DO NOTHING;",
        (clean_id.to_string(), model_type, category, provider, now),
    )
    .await?;

    let mut rows = conn
        .query(
            "UPDATE models SET \
                is_favorite = CASE WHEN is_favorite = 1 THEN 0 ELSE 1 END, \
                updated_at = ?2 \
             WHERE id = ?1 \
             RETURNING is_favorite;",
            (clean_id.to_string(), now),
        )
        .await?;

    if let Some(row) = rows.next().await? {
        let is_fav: i64 = row.get(0)?;
        Ok(is_fav != 0)
    } else {
        Ok(false)
    }
}

/// Lists all favorited model IDs ordered by updated timestamp descending.
pub async fn list_favorite_models(conn: &Connection) -> Result<Vec<String>> {
    let mut rows = conn
        .query(
            "SELECT id FROM models WHERE is_favorite = 1 ORDER BY updated_at DESC",
            (),
        )
        .await?;

    let mut ids = Vec::new();
    while let Some(row) = rows.next().await? {
        if let Ok(id) = row.get::<String>(0) {
            ids.push(id);
        }
    }
    Ok(ids)
}
