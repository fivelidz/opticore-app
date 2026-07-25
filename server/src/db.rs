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
    Ok(())
}

/// On first boot, create an admin user with a randomly-generated password and
/// print it to the log. Never hardcode credentials (fixes opticore A4/A5/B1).
pub async fn ensure_admin(pool: &SqlitePool) -> Result<()> {
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE role = 'admin'").fetch_optional(pool).await?;
    if let Some((count,)) = existing {
        if count > 0 {
            return Ok(());
        }
    }

    // In dev, allow a fixed password via DEV_ADMIN_PASSWORD (e.g. "admin") so
    // testers don't have to copy a random one. In production, leave it unset
    // and a strong random password is generated + printed once.
    let password = std::env::var("DEV_ADMIN_PASSWORD").unwrap_or_else(|_| generate_password(16));
    let hash = crate::auth::hash_password(&password)?;

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
