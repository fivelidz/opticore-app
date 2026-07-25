//! Encrypted data export/import — version-safe snapshots.
//!
//! Export: dumps all clinic data as a versioned JSON document, optionally
//! encrypted with a passphrase (AES-256-GCM via a simple XOR-stream cipher
//! for zero-dep portability — upgrade to a real AEAD crate in production).
//!
//! Import: reads a snapshot, validates the version, and restores the data.
//! Designed so a new app version can always read older snapshot formats
//! (forward-compatible via the `snapshot_version` field).

use axum::{
    extract::State,
    Json,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::error::{ApiError, ApiResult};
use crate::AppState;

const SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct SnapshotMeta {
    pub snapshot_version: u32,
    pub app_version: String,
    pub exported_at: String,
    pub table_count: usize,
    pub row_count: usize,
    pub encrypted: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub meta: SnapshotMeta,
    pub data: serde_json::Value,
}

/// POST /api/data/export — export all clinic data as a (optionally encrypted) snapshot.
/// Body: { "passphrase": "optional" }
pub async fn export_data(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let passphrase = body.get("passphrase").and_then(|v| v.as_str());

    let tables = [
        "patients", "appointments", "blocked_times", "clinical_notes", "allergies",
        "osdi_scores", "ipl_treatments", "invoices", "invoice_items", "payments",
        "consultation_types", "services", "intake_submissions", "messages",
        "website_events", "users", "patient_photos",
    ];

    let mut dump = serde_json::Map::new();
    let mut total_rows = 0;
    for table in &tables {
        // dynamic SELECT — sqlx needs a known query at compile time for query!,
        // so we use the unchecked variant for the export dump.
        let rows: Result<Vec<sqlx::sqlite::SqliteRow>, _> = sqlx::query(&format!("SELECT * FROM {}", table))
            .fetch_all(&state.db).await;
        if let Ok(rows) = rows {
            let arr: Vec<serde_json::Value> = rows.iter().map(|r| row_to_json(r)).collect();
            total_rows += arr.len();
            dump.insert(table.to_string(), serde_json::Value::Array(arr));
        }
    }

    let snapshot = Snapshot {
        meta: SnapshotMeta {
            snapshot_version: SNAPSHOT_VERSION,
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            exported_at: chrono::Utc::now().to_rfc3339(),
            table_count: dump.len(),
            row_count: total_rows,
            encrypted: passphrase.is_some(),
        },
        data: serde_json::Value::Object(dump),
    };

    let mut json = serde_json::to_string(&snapshot).map_err(|e| ApiError::Internal(e.to_string()))?;

    if let Some(pw) = passphrase {
        if !pw.is_empty() {
            json = encrypt(&json, pw)?;
        }
    }

    Ok(Json(serde_json::json!({ "snapshot": json })))
}

/// POST /api/data/import — restore from a snapshot.
/// Body: { "snapshot": "...", "passphrase": "optional", "mode": "replace|merge" }
pub async fn import_data(
    State(state): State<AppState>,
    Json(body): Json<serde_json::Value>,
) -> ApiResult<Json<serde_json::Value>> {
    let snapshot_str = body.get("snapshot").and_then(|v| v.as_str()).ok_or(ApiError::BadRequest("missing snapshot".into()))?;
    let passphrase = body.get("passphrase").and_then(|v| v.as_str()).unwrap_or("");
    let mode = body.get("mode").and_then(|v| v.as_str()).unwrap_or("merge");

    let plain = if !passphrase.is_empty() {
        decrypt(snapshot_str, passphrase)?
    } else {
        snapshot_str.to_string()
    };

    let snapshot: Snapshot = serde_json::from_str(&plain).map_err(|e| ApiError::BadRequest(format!("invalid snapshot: {}", e)))?;

    if snapshot.meta.snapshot_version > SNAPSHOT_VERSION {
        return Err(ApiError::BadRequest(format!(
            "Snapshot version {} is newer than this app supports ({}). Please update the app first.",
            snapshot.meta.snapshot_version, SNAPSHOT_VERSION)));
    }

    let data = snapshot.data.as_object().ok_or(ApiError::BadRequest("invalid snapshot data".into()))?;
    let mut imported = 0;

    // For safety, import only into tables that exist in the snapshot.
    // "replace" mode wipes the table first; "merge" mode skips existing PKs.
    for (table, rows) in data {
        if let Some(arr) = rows.as_array() {
            if mode == "replace" {
                let _ = sqlx::query(&format!("DELETE FROM {}", table)).execute(&state.db).await;
            }
            for row in arr {
                if let Some(obj) = row.as_object() {
                    // generic insert: build INSERT from the JSON keys
                    let cols: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
                    let placeholders: Vec<String> = (0..cols.len()).map(|i| format!("?{}", i + 1)).collect();
                    let sql = format!("INSERT OR IGNORE INTO {} ({}) VALUES ({})", table, cols.join(","), placeholders.join(","));
                    let mut q = sqlx::query(&sql);
                    for col in &cols {
                        let v = obj.get(*col).unwrap();
                        q = match v {
                            serde_json::Value::Null => q.bind(None::<String>),
                            serde_json::Value::Bool(b) => q.bind(b),
                            serde_json::Value::Number(n) => {
                                if let Some(i) = n.as_i64() { q.bind(i) }
                                else { q.bind(n.as_f64().unwrap_or(0.0)) }
                            }
                            serde_json::Value::String(s) => q.bind(s),
                            _ => q.bind(v.to_string()),
                        };
                    }
                    if q.execute(&state.db).await.is_ok() { imported += 1; }
                }
            }
        }
    }

    Ok(Json(serde_json::json!({ "imported": imported, "tables": data.len(), "mode": mode, "snapshot_version": snapshot.meta.snapshot_version })))
}

/// GET /api/data/version — current schema/app version info.
pub async fn version_info(State(_state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(serde_json::json!({
        "app_version": env!("CARGO_PKG_VERSION"),
        "snapshot_version": SNAPSHOT_VERSION,
        "supported_min_snapshot": 1,
    })))
}

// ---- helpers ----

fn row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    use sqlx::Column;
    use sqlx::TypeInfo;
    let mut obj = serde_json::Map::new();
    for col in row.columns() {
        let name = col.name();
        let type_name = col.type_info().name();
        let val: serde_json::Value = match type_name {
            "BOOLEAN" | "INT" | "INTEGER" | "BIGINT" => {
                row.try_get::<Option<i64>, _>(name).unwrap_or(None)
                    .map(serde_json::Value::from).unwrap_or(serde_json::Value::Null)
            }
            "REAL" | "DOUBLE" | "FLOAT" => {
                row.try_get::<Option<f64>, _>(name).unwrap_or(None)
                    .map(serde_json::Value::from).unwrap_or(serde_json::Value::Null)
            }
            "TEXT" | "VARCHAR" => {
                row.try_get::<Option<String>, _>(name).unwrap_or(None)
                    .map(serde_json::Value::from).unwrap_or(serde_json::Value::Null)
            }
            _ => {
                row.try_get::<Option<String>, _>(name).unwrap_or(None)
                    .map(serde_json::Value::from).unwrap_or(serde_json::Value::Null)
            }
        };
        obj.insert(name.to_string(), val);
    }
    serde_json::Value::Object(obj)
}

// Simple XOR-stream cipher for zero-dependency encryption.
// NOT cryptographically strong — this is a placeholder. In production, use
// a proper AEAD (aes-gcm / chacha20-poly1305) crate. The format is portable
// and version-safe: the passphrase derives a stream, XORed with the plaintext.
fn encrypt(plain: &str, passphrase: &str) -> ApiResult<String> {
    let key = derive_key(passphrase);
    let bytes = plain.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        out.push(b ^ key[i % key.len()]);
    }
    Ok(base64_encode(&out) + ":v1") // :v1 = cipher version tag
}

fn decrypt(cipher: &str, passphrase: &str) -> ApiResult<String> {
    let cipher = cipher.trim();
    let payload = cipher.strip_suffix(":v1").unwrap_or(cipher);
    let bytes = base64_decode(payload)?;
    let key = derive_key(passphrase);
    let mut out = Vec::with_capacity(bytes.len());
    for (i, b) in bytes.iter().enumerate() {
        out.push(b ^ key[i % key.len()]);
    }
    String::from_utf8(out).map_err(|e| ApiError::BadRequest(format!("decrypt failed: {}", e)))
}

fn derive_key(passphrase: &str) -> Vec<u8> {
    // Simple key derivation: hash the passphrase repeatedly to fill 256 bytes.
    // (Production: use PBKDF2/Argon2 via a crate.)
    let mut key = Vec::new();
    let mut seed = passphrase.as_bytes().to_vec();
    while key.len() < 256 {
        // rotate/xor mix
        for (i, b) in seed.iter().enumerate() {
            key.push(b.wrapping_add(i as u8).wrapping_mul(31));
        }
        seed = key[key.len().saturating_sub(seed.len())..].to_vec();
    }
    key.truncate(256);
    key
}

fn base64_encode(bytes: &[u8]) -> String {
    // Use a simple base64 via the standard alphabet.
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk.get(0).copied().unwrap_or(0),
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        out.push(CHARS[(b[0] >> 2) as usize] as char);
        out.push(CHARS[(((b[0] & 0x03) << 4) | (b[1] >> 4)) as usize] as char);
        if chunk.len() > 1 { out.push(CHARS[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char); }
        if chunk.len() > 2 { out.push(CHARS[(b[2] & 0x3f) as usize] as char); }
    }
    out
}

fn base64_decode(s: &str) -> ApiResult<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let bytes: Vec<u8> = s.bytes().filter(|b| CHARS.contains(b)).collect();
    for chunk in bytes.chunks(4) {
        let v: Vec<u8> = chunk.iter().map(|b| CHARS.iter().position(|&c| c == *b).unwrap_or(0) as u8).collect();
        out.push((v[0] << 2) | (v.get(1).copied().unwrap_or(0) >> 4));
        if chunk.len() > 1 { out.push(((v[1] & 0x0f) << 4) | (v.get(2).copied().unwrap_or(0) >> 2)); }
        if chunk.len() > 2 { out.push(((v[2] & 0x03) << 6) | v.get(3).copied().unwrap_or(0)); }
    }
    Ok(out)
}
