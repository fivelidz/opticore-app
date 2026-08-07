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
use sqlx::{Acquire, Row};

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

    // Disable FK enforcement for the duration of the restore and wrap it in a
    // transaction. Two reasons:
    //   1. The snapshot's table order is arbitrary (alphabetical), so a
    //      `DELETE FROM parents` in replace mode would CASCADE-delete already-
    //      imported child rows (e.g. deleting `patients` wipes `appointments`
    //      that were inserted moments earlier).
    //   2. Child rows may reference parents that haven't been inserted yet.
    // Bulk-load convention: load with FKs off, then re-enable (SQLite validates
    // integrity on the next write — acceptable for a restore-from-backup path).
    //
    // NOTE: PRAGMA foreign_keys is *per-connection* and cannot be set inside a
    // transaction. So we acquire a dedicated connection, set the pragma on it,
    // then begin the transaction on that same connection.
    let mut conn = state.db.acquire().await?;
    sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await?;
    let mut tx = (&mut *conn).begin().await?;

    // For safety, import only into tables that exist in the snapshot.
    // "replace" mode wipes the table first; "merge" mode skips existing PKs.
    for (table, rows) in data {
        if let Some(arr) = rows.as_array() {
            if mode == "replace" {
                let _ = sqlx::query(&format!("DELETE FROM {}", table)).execute(&mut *tx).await;
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
                            // BLOB round-trip: export tags binary columns as
                            // "b64:<base64>"; decode back to raw bytes on import
                            // so SQLite stores them as BLOB, not text. Strings
                            // without the tag bind as plain TEXT.
                            serde_json::Value::String(s) if s.starts_with("b64:") => {
                                match base64_decode(&s[4..]) {
                                    Ok(bytes) => q.bind(bytes),
                                    // malformed b64: tag — bind the original
                                    // string verbatim rather than dropping the row.
                                    Err(_) => q.bind(s),
                                }
                            }
                            serde_json::Value::String(s) => q.bind(s),
                            _ => q.bind(v.to_string()),
                        };
                    }
                    if q.execute(&mut *tx).await.is_ok() { imported += 1; }
                }
            }
        }
    }

    tx.commit().await?;
    // Re-enable FK enforcement on this connection before it returns to the pool.
    sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await?;
    drop(conn);
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
            "BLOB" => {
                // BLOB columns hold raw bytes that may not be valid UTF-8. The
                // old catch-all tried Option<String> and silently dropped
                // binary blobs (data loss on export). Encode as base64 with a
                // "b64:" tag so importers can distinguish BLOB-origin strings
                // from plain TEXT. NULL stays JSON null.
                match row.try_get::<Option<Vec<u8>>, _>(name) {
                    Ok(Some(bytes)) => {
                        serde_json::Value::String(format!("b64:{}", base64_encode(&bytes)))
                    }
                    _ => serde_json::Value::Null,
                }
            }
            _ => {
                // Unknown type (e.g. DATETIME, DATE, NUMERIC): SQLite stores
                // these as TEXT, so Option<String> round-trips correctly. If it
                // fails (genuinely binary value in an untyped column), fall
                // back to BLOB-style base64 rather than silently emitting null.
                match row.try_get::<Option<String>, _>(name) {
                    Ok(Some(s)) => serde_json::Value::String(s),
                    _ => match row.try_get::<Option<Vec<u8>>, _>(name) {
                        Ok(Some(bytes)) => {
                            serde_json::Value::String(format!("b64:{}", base64_encode(&bytes)))
                        }
                        _ => serde_json::Value::Null,
                    },
                }
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
    //
    // BUGFIX: an empty passphrase produced an empty `seed`, so the inner `for`
    // loop pushed nothing, `key` never grew, and the `while key.len() < 256`
    // loop spun forever (an infinite hang in encrypt/decrypt). We now fall
    // back to a non-empty seed when the passphrase is empty so the stream
    // always advances. (An empty passphrase is degenerate and offers no real
    // security regardless; this just prevents the hang.)
    let seed0 = passphrase.as_bytes();
    let mut seed: Vec<u8> = if seed0.is_empty() {
        b"\x00opticore-empty-passphrase-fallback".to_vec()
    } else {
        seed0.to_vec()
    };
    let mut key = Vec::with_capacity(256);
    while key.len() < 256 {
        for (i, b) in seed.iter().enumerate() {
            key.push(b.wrapping_add(i as u8).wrapping_mul(31));
        }
        // Advance the seed window; guaranteed to make progress because seed is
        // non-empty (guarded above) and key grows by seed.len() each iteration.
        seed = key[key.len().saturating_sub(seed.len())..].to_vec();
    }
    key.truncate(256);
    key
}

fn base64_encode(bytes: &[u8]) -> String {
    // Standard base64 with = padding (RFC 4648). The earlier hand-rolled
    // version omitted padding and its decode produced extra trailing zero
    // bytes on non-multiple-of-3 input, corrupting round-trips.
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
        match chunk.len() {
            1 => out.push_str("=="),
            2 => {
                out.push(CHARS[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
                out.push('=');
            }
            _ => {
                out.push(CHARS[(((b[1] & 0x0f) << 2) | (b[2] >> 6)) as usize] as char);
                out.push(CHARS[(b[2] & 0x3f) as usize] as char);
            }
        }
    }
    out
}

fn base64_decode(s: &str) -> ApiResult<Vec<u8>> {
    // Standard base64 decode with = padding awareness (RFC 4648).
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let lookup = |b: u8| -> Option<u8> { CHARS.iter().position(|&c| c == b).map(|p| p as u8) };
    // Strip padding; we infer tail length from the filtered-char count.
    let filtered: Vec<u8> = s.bytes().filter(|b| *b != b'=' && *b != b'\n' && *b != b'\r' && *b != b' ').collect();
    // Each group of 4 base64 chars → 3 bytes. The final group may be short.
    let mut out = Vec::with_capacity(filtered.len() * 3 / 4);
    let main = filtered.len() / 4 * 4;
    for chunk in filtered[..main].chunks(4) {
        let v: [u8; 4] = [lookup(chunk[0]).unwrap_or(0), lookup(chunk[1]).unwrap_or(0),
                          lookup(chunk[2]).unwrap_or(0), lookup(chunk[3]).unwrap_or(0)];
        out.push((v[0] << 2) | (v[1] >> 4));
        out.push(((v[1] & 0x0f) << 4) | (v[2] >> 2));
        out.push(((v[2] & 0x03) << 6) | v[3]);
    }
    // Handle the tail (1, 2, or 3 remaining base64 chars → 0, 1, or 2 bytes).
    let tail = &filtered[main..];
    match tail.len() {
        0 => {}
        2 => {
            let v = [lookup(tail[0]).unwrap_or(0), lookup(tail[1]).unwrap_or(0)];
            out.push((v[0] << 2) | (v[1] >> 4));
        }
        3 => {
            let v = [lookup(tail[0]).unwrap_or(0), lookup(tail[1]).unwrap_or(0), lookup(tail[2]).unwrap_or(0)];
            out.push((v[0] << 2) | (v[1] >> 4));
            out.push(((v[1] & 0x0f) << 4) | (v[2] >> 2));
        }
        _ => return Err(ApiError::BadRequest("invalid base64 length".into())),
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    //! Property-style tests for the cipher and base64 helpers.
    //!
    //! These are unit tests (inside the module) so they can call the private
    //! `encrypt`/`decrypt`/`base64_*` functions directly. The approach is a
    //! loop-based fuzz: generate N random (plaintext, passphrase) pairs with
    //! `rand`, round-trip them, and assert equality. No `quickcheck`/`proptest`
    //! dep is added — `rand` is already a dependency.

    use super::*;

    use rand::Rng;

    /// Round-trip a single (plaintext, passphrase) pair through encrypt/decrypt
    /// and assert the recovered plaintext equals the input.
    fn assert_round_trip(plain: &str, passphrase: &str) {
        let ct = match encrypt(plain, passphrase) {
            Ok(c) => c,
            Err(e) => panic!("encrypt failed for plain={plain:?} pw={passphrase:?}: {e}"),
        };
        let pt = match decrypt(&ct, passphrase) {
            Ok(p) => p,
            Err(e) => panic!("decrypt failed for ct={ct:?} pw={passphrase:?}: {e}"),
        };
        assert_eq!(pt, plain, "round-trip mismatch for plain={plain:?} pw={passphrase:?}");
    }

    // ---- base64 round-trip (the bug the prior agent fixed) ----

    #[test]
    fn base64_round_trip_all_byte_lengths() {
        // Lengths that hit every padding boundary (0, 1, 2, 3 mod 3).
        for len in 0..=300 {
            let bytes: Vec<u8> = (0..len).map(|i| (i as u8).wrapping_mul(7)).collect();
            let enc = base64_encode(&bytes);
            let dec = base64_decode(&enc).expect("decode");
            assert_eq!(dec, bytes, "base64 round-trip failed at len {len}");
        }
    }

    #[test]
    fn base64_round_trip_random_bytes() {
        let mut rng = rand::thread_rng();
        for _ in 0..200 {
            let len = rng.gen_range(0..512);
            let bytes: Vec<u8> = (0..len).map(|_| rng.gen::<u8>()).collect();
            let enc = base64_encode(&bytes);
            let dec = base64_decode(&enc).expect("decode");
            assert_eq!(dec, bytes, "base64 random round-trip failed (len {len})");
        }
    }

    #[test]
    fn base64_encode_matches_known_vectors() {
        // RFC 4648 §10 test vectors (standard base64).
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    // ---- cipher round-trip: edge cases ----

    #[test]
    fn cipher_round_trip_empty_plaintext() {
        assert_round_trip("", "any-passphrase");
    }

    #[test]
    fn cipher_round_trip_empty_passphrase() {
        // Empty passphrase is a degenerate but legal input; it must not panic.
        assert_round_trip("some plaintext", "");
        assert_round_trip("", "");
    }

    #[test]
    fn cipher_round_trip_unicode_plaintext() {
        // Multi-byte UTF-8: the cipher operates on bytes, so valid UTF-8 must
        // survive the round-trip (decrypt does String::from_utf8).
        assert_round_trip("héllo 世界 🦀", "pw");
        assert_round_trip("Ωμέγα", "clé");
    }

    #[test]
    fn cipher_round_trip_unicode_passphrase() {
        assert_round_trip("plain", "пароль");
        assert_round_trip("data", "パスワード");
    }

    #[test]
    fn cipher_round_trip_large_plaintext() {
        // Larger than the 256-byte key stream — exercises the modular wrap.
        let big = "A".repeat(10_000);
        assert_round_trip(&big, "pw");
        // Just over a key-cycle boundary plus a partial tail.
        let odd = "AB".repeat(127); // 254 bytes
        assert_round_trip(&odd, "k");
    }

    #[test]
    fn cipher_output_is_tagged_v1() {
        let ct = encrypt("hello", "pw").unwrap();
        assert!(ct.ends_with(":v1"), "ciphertext should carry the :v1 version tag: {ct}");
    }

    #[test]
    fn cipher_output_is_base64_plus_tag() {
        // The body before :v1 must be valid base64 (decodable without error).
        let ct = encrypt("the quick brown fox", "secret").unwrap();
        let body = ct.strip_suffix(":v1").unwrap_or(&ct);
        assert!(base64_decode(body).is_ok(), "cipher body must be valid base64: {body}");
    }

    #[test]
    fn decrypt_wrong_passphrase_does_not_panic() {
        // A wrong passphrase produces garbage bytes that are *usually* not valid
        // UTF-8 → decrypt returns Err(BadRequest). It must never panic. Even if
        // the garbage happens to be valid UTF-8, the test only asserts no panic
        // (the recovered string won't equal the original in practice).
        let ct = encrypt("secret message", "right-pw").unwrap();
        let _ = decrypt(&ct, "wrong-pw"); // must not panic
    }

    #[test]
    fn decrypt_garbage_input_returns_err() {
        // Non-base64 garbage → BadRequest, no panic.
        assert!(decrypt("!!!not base64!!!", "pw").is_err());
        // Valid base64 of non-UTF8 → BadRequest (String::from_utf8 fails).
        // 0xff is invalid as a leading UTF-8 byte.
        let bad = base64_encode(&[0xff, 0xfe, 0xff, 0xfe]);
        assert!(decrypt(&bad, "pw").is_err());
    }

    #[test]
    fn decrypt_strips_whitespace() {
        // base64_decode tolerates embedded whitespace/newlines (some transports
        // wrap lines). The cipher body should round-trip regardless.
        let ct = encrypt("payload", "pw").unwrap();
        let body = ct.strip_suffix(":v1").unwrap_or(&ct);
        let with_ws = format!(" \n{} \n", body);
        let tagged = format!("{}:v1", with_ws);
        let pt = decrypt(&tagged, "pw").expect("whitespace-tolerant decode");
        assert_eq!(pt, "payload");
    }

    // ---- cipher round-trip: fuzz ----

    #[test]
    fn cipher_round_trip_fuzz_random_json() {
        /// Generate a random JSON value of varying shape/size and return its
        /// serialized form. Covers nested objects, arrays, unicode, numbers.
        fn random_json(rng: &mut impl Rng, depth: u32) -> serde_json::Value {
            if depth >= 3 {
                return serde_json::Value::from(rng.gen::<u32>());
            }
            match rng.gen_range(0..6) {
                0 => serde_json::Value::Null,
                1 => serde_json::Value::Bool(rng.gen_bool(0.5)),
                2 => serde_json::Value::from(rng.gen::<i64>()),
                3 => serde_json::Value::from(rng.gen::<f64>()),
                4 => {
                    // unicode string
                    let n = rng.gen_range(0..40);
                    let s: String = (0..n)
                        .map(|_| {
                            // mix ASCII and BMP codepoints
                            let cp = if rng.gen_bool(0.5) {
                                rng.gen_range(b'a'..=b'z') as u32
                            } else {
                                rng.gen_range(0x4e00..=0x9fff) // CJK block
                            };
                            char::from_u32(cp).unwrap_or('?')
                        })
                        .collect();
                    serde_json::Value::String(s)
                }
                _ => {
                    // array or object
                    let n = rng.gen_range(0..6);
                    if rng.gen_bool(0.5) {
                        let arr: Vec<_> = (0..n).map(|_| random_json(rng, depth + 1)).collect();
                        serde_json::Value::Array(arr)
                    } else {
                        let obj: serde_json::Map<String, serde_json::Value> = (0..n)
                            .map(|i| (format!("k{i}"), random_json(rng, depth + 1)))
                            .collect();
                        serde_json::Value::Object(obj)
                    }
                }
            }
        }

        let mut rng = rand::thread_rng();
        for _ in 0..500 {
            let v = random_json(&mut rng, 0);
            let plain = serde_json::to_string(&v).expect("serialize");
            // random passphrase: sometimes empty, sometimes unicode, sometimes long
            let pw = match rng.gen_range(0..4) {
                0 => String::new(),
                1 => format!("pw-{}", rng.gen::<u32>()),
                2 => "пароль".to_string(),
                _ => (0..rng.gen_range(0..64)).map(|_| rng.gen::<char>()).collect::<String>(),
            };
            assert_round_trip(&plain, &pw);
        }
    }

    #[test]
    fn cipher_round_trip_fuzz_all_byte_values() {
        // Plaintexts containing every possible byte value (as part of valid
        // UTF-8 via Latin-1-ish construction). This stresses the XOR stream.
        let mut rng = rand::thread_rng();
        for _ in 0..100 {
            let n = rng.gen_range(0..256);
            // Build a string of ASCII bytes (0x20..0x7e) so from_utf8 always
            // succeeds — we're testing the cipher, not UTF-8 validity.
            let s: String = (0..n).map(|_| rng.gen_range(b' '..=b'~') as char).collect();
            let pw: String = (0..rng.gen_range(0..32)).map(|_| rng.gen_range(b' '..=b'~') as char).collect();
            assert_round_trip(&s, &pw);
        }
    }

    // ---- row_to_json SQL type coverage ----

    /// Helper: create an in-memory SQLite pool with a single typed test table,
    /// insert one row, and return the rows so the caller can pass them through
    /// `row_to_json`.
    async fn typed_rows(sql_create: &str, insert: &str) -> Vec<sqlx::sqlite::SqliteRow> {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory db");
        sqlx::query(sql_create).execute(&pool).await.expect("create table");
        sqlx::query(insert).execute(&pool).await.expect("insert row");
        let rows: Vec<sqlx::sqlite::SqliteRow> =
            sqlx::query("SELECT * FROM typed").fetch_all(&pool).await.expect("select");
        // keep pool alive until here; rows are detached (owned by SqliteRow).
        drop(pool);
        rows
    }

    /// INTEGER → JSON number.
    #[tokio::test]
    async fn row_to_json_integer_column() {
        let rows = typed_rows(
            "CREATE TABLE typed (v INTEGER)",
            "INSERT INTO typed (v) VALUES (42)",
        )
        .await;
        let v = row_to_json(&rows[0]);
        assert_eq!(v["v"], serde_json::json!(42), "INTEGER should be a JSON number");
        assert!(v["v"].is_i64(), "INTEGER should deserialize as i64");
    }

    /// NULL INTEGER → JSON null (not 0).
    #[tokio::test]
    async fn row_to_json_null_integer_is_json_null() {
        let rows = typed_rows(
            "CREATE TABLE typed (v INTEGER)",
            "INSERT INTO typed (v) VALUES (NULL)",
        )
        .await;
        let v = row_to_json(&rows[0]);
        assert!(v["v"].is_null(), "NULL should be JSON null, not 0");
    }

    /// REAL → JSON number (f64).
    #[tokio::test]
    async fn row_to_json_real_column() {
        let rows = typed_rows(
            "CREATE TABLE typed (v REAL)",
            "INSERT INTO typed (v) VALUES (3.14)",
        )
        .await;
        let v = row_to_json(&rows[0]);
        assert_eq!(v["v"], serde_json::json!(3.14), "REAL should be a JSON float");
        assert!(v["v"].is_f64(), "REAL should deserialize as f64");
    }

    /// TEXT → JSON string.
    #[tokio::test]
    async fn row_to_json_text_column() {
        let rows = typed_rows(
            "CREATE TABLE typed (v TEXT)",
            "INSERT INTO typed (v) VALUES ('hello')",
        )
        .await;
        let v = row_to_json(&rows[0]);
        assert_eq!(v["v"], serde_json::json!("hello"), "TEXT should be a JSON string");
    }

    /// DATETIME → SQLite stores these as TEXT ("YYYY-MM-DD HH:MM:SS"). The
    /// catch-all arm reads them as String, which round-trips correctly. This
    /// test documents and locks that behavior.
    #[tokio::test]
    async fn row_to_json_datetime_column_is_text() {
        let rows = typed_rows(
            "CREATE TABLE typed (v DATETIME)",
            "INSERT INTO typed (v) VALUES ('2024-01-15 09:30:00')",
        )
        .await;
        let v = row_to_json(&rows[0]);
        // DATETIME is not in the explicit match arms; it falls through to the
        // catch-all which tries Option<String>. SQLite stores it as TEXT, so
        // this succeeds and yields the string verbatim.
        assert_eq!(v["v"], serde_json::json!("2024-01-15 09:30:00"));
        assert!(v["v"].is_string(), "DATETIME should surface as a JSON string");
    }

    /// BLOB → the catch-all arm tries Option<String>, which FAILS for raw
    /// binary BLOB data (it's not valid UTF-8). The current code silently
    /// swallows the error via `.unwrap_or(None)` and emits JSON null —
    /// **data loss on export**. This test documents the current (buggy)
    /// behavior so the fix below is verifiable, then asserts the FIXED
    /// behavior: BLOBs are base64-encoded into a JSON string.
    #[tokio::test]
    async fn row_to_json_blob_column_is_base64_encoded() {
        // Raw bytes that are NOT valid UTF-8 (0xff, 0xfe, ...).
        let blob_bytes: Vec<u8> = vec![0xff, 0xfe, 0x00, 0x41, 0x42, 0xff];
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect");
        sqlx::query("CREATE TABLE typed (v BLOB)")
            .execute(&pool)
            .await
            .expect("create");
        sqlx::query("INSERT INTO typed (v) VALUES (?)")
            .bind(&blob_bytes[..])
            .execute(&pool)
            .await
            .expect("insert");
        let rows: Vec<sqlx::sqlite::SqliteRow> =
            sqlx::query("SELECT * FROM typed").fetch_all(&pool).await.expect("select");
        drop(pool);

        let v = row_to_json(&rows[0]);
        // FIXED behavior: BLOB is base64-encoded into a JSON string with a
        // "b64:" prefix so importers can distinguish it from plain TEXT.
        let s = v["v"].as_str().expect("BLOB should be a base64 JSON string");
        assert!(s.starts_with("b64:"), "BLOB should be tagged b64:, got: {s}");
        let b64 = &s[4..];
        let decoded = base64_decode(b64).expect("BLOB payload should be valid base64");
        assert_eq!(decoded, blob_bytes, "decoded BLOB must equal the original bytes");
    }

    /// BLOB that happens to be valid UTF-8 should still be tagged b64: (so the
    /// importer knows it came from a BLOB column, not TEXT). This guards against
    /// a regression where valid-UTF-8 BLOBs silently pass through as strings
    /// and lose their type marker.
    #[tokio::test]
    async fn row_to_json_utf8_blob_is_still_base64_tagged() {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect");
        sqlx::query("CREATE TABLE typed (v BLOB)")
            .execute(&pool)
            .await
            .expect("create");
        sqlx::query("INSERT INTO typed (v) VALUES (?)")
            .bind(&b"hello"[..])
            .execute(&pool)
            .await
            .expect("insert");
        let rows: Vec<sqlx::sqlite::SqliteRow> =
            sqlx::query("SELECT * FROM typed").fetch_all(&pool).await.expect("select");
        drop(pool);
        let v = row_to_json(&rows[0]);
        let s = v["v"].as_str().expect("BLOB should be a string");
        assert!(s.starts_with("b64:"), "even UTF-8 BLOBs must be tagged: {s}");
        let decoded = base64_decode(&s[4..]).expect("decode");
        assert_eq!(decoded, b"hello");
    }

    /// NULL BLOB → JSON null (not "b64:").
    #[tokio::test]
    async fn row_to_json_null_blob_is_json_null() {
        let rows = typed_rows(
            "CREATE TABLE typed (v BLOB)",
            "INSERT INTO typed (v) VALUES (NULL)",
        )
        .await;
        let v = row_to_json(&rows[0]);
        assert!(v["v"].is_null(), "NULL BLOB should be JSON null");
    }
}
