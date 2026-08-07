//! Database pool, migrations, and first-boot admin provisioning.

use anyhow::{anyhow, Result};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    SqlitePool,
};
use tracing::{info, warn};

pub async fn init_pool(url: &str) -> Result<SqlitePool> {
    // sqlx expects "sqlite://" for in-file; ensure mode=rwc so it's created.
    let url = if url.contains('?') {
        url.to_string()
    } else {
        format!("{}?mode=rwc", url)
    };

    // Build connect options from the URL and pin the pragmas we rely on.
    //
    // `foreign_keys = ON` is the most important: it makes SQLite actually
    // enforce the `FOREIGN KEY ... ON DELETE CASCADE` declarations in the
    // schema (appointments/invoices/clinical_notes → patients, invoice_items
    // → invoices, payments → invoices). Without it, deleting a patient would
    // silently orphan every dependent row — lost clinical/financial history.
    //
    // sqlx 0.8 enables `foreign_keys = ON` by default, but that is an
    // implicit library default we should not depend on: a future sqlx
    // upgrade, a different connection path, or a URL query param could
    // silently flip it off. Setting it explicitly here makes the
    // referential-integrity guarantee independent of sqlx's defaults and
    // self-documenting. (The data_io import path still toggles it OFF
    // per-connection for ordered bulk restore — that override still wins
    // because it runs on the connection after this default is applied.)
    //
    // `journal_mode = WAL` + `synchronous = NORMAL` + `busy_timeout = 5s`
    // are essential for multi-device / multi-client concurrent access:
    //
    //   - WAL (Write-Ahead Logging) allows readers and a writer to coexist
    //     without blocking each other. Under the default rollback-journal
    //     mode, a write locks the entire database and concurrent reads (from
    //     another device's request) fail with "database is locked". WAL lets
    //     multiple LAN clients read while one writes — the common case for a
    //     small clinic with several tablets/desktops hitting the same server.
    //   - `synchronous = NORMAL` is the recommended companion to WAL: it's
    //     nearly as safe as FULL (the WAL file is still fsync'd on checkpoint)
    //     but dramatically faster, especially under concurrent write pressure.
    //   - `busy_timeout = 5s` makes a connection that hits a lock WAIT up to
    //     5 seconds before returning SQLITE_BUSY, instead of failing
    //     instantly. This absorbs the brief contention windows that occur
    //     when two requests try to write at the same instant.
    //
    // The URL carries `?mode=rwc` which already sets `create_if_missing`, so
    // we don't set it again here.
    let connect_opts: SqliteConnectOptions = url.parse()?;
    let connect_opts = connect_opts
        .foreign_keys(true)
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
        .busy_timeout(std::time::Duration::from_secs(5));

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(connect_opts)
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

/// Re-seed "structural invariant" tables that must not be empty.
///
/// The data_io replace-mode import (`POST /api/data/import` with
/// `mode: "replace"`) does `DELETE FROM <table>` for every table present in
/// the snapshot, then inserts the snapshot's rows. If a structural table is
/// in the snapshot with an **empty array**, it is wiped and not repopulated.
/// That can brick the system:
///
///   - `users` with no admin row → no one can log in (no way to recover via
///     the API; only out-of-band DB surgery or a re-seed can restore access).
///   - `consultation_types` / `services` empty → the billing/booking catalogs
///     are gone; staff can't book or bill until re-seeded.
///
/// (`booking_settings` is NOT in the import allowlist, so replace-mode never
/// deletes it; its own lazy-init in `load_settings` covers the out-of-band
/// DELETE case.)
///
/// This function is called after a replace-mode import commits. For each
/// structural table that ended up empty, it re-runs the same seed the
/// migration installed (idempotent via `INSERT OR IGNORE` / count guards).
/// The admin is re-provisioned with a fresh random password (printed to the
/// log, exactly like first boot) — there is no way to recover the old hash
/// from an empty table.
pub async fn reseed_structural_invariants(pool: &SqlitePool) -> Result<()> {
    // --- admin user ---
    // If the import wiped all admins, re-create one with a fresh random
    // password. We cannot recover the original password hash, so this is a
    // "break-glass" re-seed: the operator must read the new password from the
    // server log and change it after login.
    let admin_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE role = 'admin'")
            .fetch_one(pool)
            .await?;
    if admin_count == 0 {
        let pw = generate_password(20);
        provision_admin(pool, &pw).await?;
        warn!("replace-mode import left users table with no admin; re-seeded a fresh admin (see credentials above). Change the password after login.");
    }

    // --- consultation_types catalog ---
    // Re-run the 0003 seed. INSERT OR IGNORE makes this safe if rows survived.
    let ct_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM consultation_types")
            .fetch_one(pool)
            .await?;
    if ct_count == 0 {
        sqlx::query(
            "INSERT OR IGNORE INTO consultation_types (type_code, type_name, description, default_price, default_duration_minutes, medicare_item_number) VALUES
             ('DRY-EYE', 'Dry Eye Consultation', 'Comprehensive dry eye assessment', 350, 60, '10910'),
             ('FOLLOWUP', 'Follow-up', 'Review consultation', 150, 30, '10912'),
             ('IPL', 'IPL Treatment', 'Intense Pulsed Light therapy session', 300, 45, NULL),
             ('IMAGING', 'Imaging', 'Diagnostic imaging session', 250, 30, NULL),
             ('TELEHEALTH', 'Telehealth', 'Remote consultation', 120, 20, '91852')",
        )
        .execute(pool)
        .await?;
        warn!("replace-mode import left consultation_types empty; re-seeded default catalog.");
    }

    // --- services catalog ---
    let svc_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM services")
            .fetch_one(pool)
            .await?;
    if svc_count == 0 {
        sqlx::query(
            "INSERT OR IGNORE INTO services (service_code, service_name, category, description, unit_price, unit_type, tax_rate) VALUES
             ('IPL-SESSION', 'IPL Therapy Session', 'treatment', 'Single IPL treatment', 300, 'each', 0.10),
             ('OPTILIGHT', 'OptiLight Treatment', 'treatment', 'IPL OptiLight session', 275, 'each', 0.10),
             ('KERATO-5M', 'Keratograph 5M Imaging', 'imaging', 'Ocular surface imaging', 180, 'each', 0.10),
             ('LIPIVIEW', 'LipiView II', 'imaging', 'Lipid layer imaging', 200, 'each', 0.10),
             ('TBUT', 'Tear Break-Up Time', 'imaging', 'TBUT test', 90, 'each', 0.10),
             ('RESTASIS', 'Restasis 0.05%', 'medication', 'Cyclosporine emulsion', 95, 'each', 0.10),
             ('XIIDRA', 'Xiidra 5%', 'medication', 'Lifitegrast eye drops', 110, 'each', 0.10),
             ('ARTIFICIAL', 'Artificial Tears', 'medication', 'Lubricant drops', 25, 'each', 0.10),
             ('WARM-COMP', 'Warm Compress', 'supply', 'Eye mask', 35, 'each', 0.10),
             ('SUPPLY-OTHER', 'Other Supply', 'supply', 'Misc supply item', 20, 'each', 0.10)",
        )
        .execute(pool)
        .await?;
        warn!("replace-mode import left services empty; re-seeded default catalog.");
    }

    Ok(())
}
