//! Database pool, migrations, and first-boot admin provisioning.

use anyhow::{anyhow, Result};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use tracing::{info, warn};

pub async fn init_pool(url: &str) -> Result<SqlitePool> {
    // sqlx expects "sqlite://" for in-file; ensure mode=rwc so it's created.
    let opts = if url.contains('?') {
        url.to_string()
    } else {
        format!("{}?mode=rwc", url)
    };
    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect(&opts)
        .await?;
    Ok(pool)
}

pub async fn run_migrations(pool: &SqlitePool) -> Result<()> {
    // Migrations are embedded at compile time.
    sqlx::migrate!("./migrations").run(pool).await.map_err(|e| anyhow!("migrate: {e}"))?;
    info!("✓ migrations applied");

    // If CLEAN_START is set, wipe all demo/seed data so the app starts empty.
    // This is used for production builds where you want a blank database.
    //
    // SAFETY: this must only EVER run once, on a brand-new database. Otherwise a
    // production launcher that always sets CLEAN_START would delete real patient
    // data on every restart. We record a marker row the first time and skip the
    // wipe forever after, so real data is never touched on subsequent boots.
    let clean_requested = std::env::var("CLEAN_START").is_ok();
    let already_cleaned: bool = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM app_meta WHERE key = 'clean_started'")
        .fetch_one(pool).await.unwrap_or(0) > 0;
    if clean_requested && !already_cleaned {
        info!("🧹 CLEAN_START: wiping demo data for fresh production start...");
        let tables = [
            "clinical_notes", "allergies", "osdi_scores", "ipl_treatments",
            "invoice_items", "invoices", "payments", "appointments",
            "blocked_times", "patient_photos", "intake_submissions",
            "messages", "website_events", "patients",
            // keep users (admin), consultation_types, services, booking_settings
        ];
        for t in &tables {
            sqlx::query(&format!("DELETE FROM {}", t)).execute(pool).await?;
        }
        // reset the auto-increment counters
        sqlx::query("DELETE FROM sqlite_sequence WHERE name IN ('patients','appointments','invoices','invoice_items','payments','clinical_notes','allergies','osdi_scores','ipl_treatments','blocked_times','intake_submissions','messages','website_events','patient_photos')").execute(pool).await?;
        // Record that this database has been cleaned so we never wipe it again,
        // even if CLEAN_START stays set on every launch.
        sqlx::query("INSERT OR REPLACE INTO app_meta (key, value) VALUES ('clean_started', datetime('now'))").execute(pool).await?;
        info!("✓ demo data cleared — app starts empty (only admin user + catalogs remain)");
    } else if clean_requested && already_cleaned {
        info!("CLEAN_START ignored — this database was already initialised; real data preserved.");
    }

    Ok(())
}

/// On first boot, create an admin user with a randomly-generated password and
/// print it to the log. Never hardcode credentials (fixes opticore A4/A5/B1).
///
/// Production path: reads `DEV_ADMIN_PASSWORD` from the env (or generates a
/// strong random one if unset). Tests should call [`ensure_admin_with_password`]
/// instead, to avoid mutating the process-global env var (which races under
/// parallel `cargo test`).
pub async fn ensure_admin(pool: &SqlitePool) -> Result<()> {
    // In dev, allow a fixed password via DEV_ADMIN_PASSWORD (e.g. "admin") so
    // testers don't have to copy a random one. In production, leave it unset
    // and a strong random password is generated + printed once.
    let password = std::env::var("DEV_ADMIN_PASSWORD").unwrap_or_else(|_| generate_password(16));
    provision_admin(pool, &password).await
}

/// Provision the first-boot admin with an explicit password.
///
/// This is the test-friendly entry point: it does NOT touch the process-global
/// `DEV_ADMIN_PASSWORD` env var, so it is safe to call from many parallel test
/// tasks. If an admin already exists, this is a no-op (matches `ensure_admin`).
pub async fn ensure_admin_with_password(pool: &SqlitePool, password: &str) -> Result<()> {
    provision_admin(pool, password).await
}

/// Shared body of [`ensure_admin`] / [`ensure_admin_with_password`]: insert the
/// admin row if none exists, using the supplied plaintext `password`.
async fn provision_admin(pool: &SqlitePool, password: &str) -> Result<()> {
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'").fetch_optional(pool).await?;
    if let Some((count,)) = existing {
        if count > 0 {
            return Ok(());
        }
    }

    let hash = crate::auth::hash_password(password)?;

    sqlx::query(
        "INSERT INTO users (username, email, password_hash, role, first_name, last_name, is_active)
         VALUES ('admin', 'admin@clinic.local', ?, 'admin', 'System', 'Administrator', 1)",
    )
    .bind(&hash)
    .execute(pool)
    .await?;

    // Print loudly so the installer/user sees it once.
    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    warn!("🔑 FIRST-BOOT ADMIN CREDENTIALS (set a new password after login):");
    warn!("   username: admin");
    warn!("   password: {}", password);
    warn!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    info!("admin user created");
    Ok(())
}

fn generate_password(len: usize) -> String {
    use rand::Rng;
    const UPPER: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijkmnopqrstuvwxyz";
    const DIGIT: &[u8] = b"23456789";
    const SYM: &[u8] = b"!@#$%^&*";
    let mut rng = rand::thread_rng();
    let mut s = String::new();
    for _ in 0..len {
        let set = rng.gen_range(0..4);
        let mut pick = |a: &[u8]| a[rng.gen_range(0..a.len())] as char;
        s.push(match set {
            0 => pick(UPPER),
            1 => pick(LOWER),
            2 => pick(DIGIT),
            _ => pick(SYM),
        });
    }
    s
}
