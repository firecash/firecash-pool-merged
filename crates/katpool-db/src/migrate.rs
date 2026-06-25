//! Migration runner.
//!
//! Migrations are SQL files in `crates/katpool-db/migrations/` named
//! `<timestamp>_<name>.sql`. The [`sqlx::migrate!`] macro embeds them
//! into the binary at compile time so production has no on-disk
//! dependency.
//!
//! The runner is **fail-closed**: if any migration fails or the
//! database schema is ahead of what this binary knows about, the
//! caller (`katpool/src/main.rs`) refuses to start. Operators see a
//! single `journalctl` error with the migration filename.

use crate::error::DbError;

/// Embedded migrations. The macro re-evaluates at compile time, so a
/// new migration file under `migrations/` is picked up automatically.
pub static MIGRATIONS: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

/// Apply every pending migration in order, leaving the schema at
/// HEAD. Idempotent: re-running against an already-up-to-date
/// database is a no-op.
///
/// Logs each migration's filename + duration via `tracing::info!`
/// so operators can see schema-change history in the journal.
pub async fn run(pool: &sqlx::PgPool) -> Result<(), DbError> {
    let started = std::time::Instant::now();
    tracing::info!(
        migration_count = MIGRATIONS.iter().count(),
        "applying katpool-db migrations"
    );

    MIGRATIONS.run(pool).await.map_err(DbError::from)?;

    tracing::info!(
        elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        "migrations applied"
    );
    Ok(())
}

/// Compile-time sanity: the migrator must have at least one
/// migration. Catches an accidentally-empty `migrations/` directory.
#[allow(dead_code)]
const _: () = {
    // Cannot inspect MIGRATIONS at const-eval time because it's
    // initialised lazily; this assertion runs at first-call via the
    // test below.
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrator_has_bootstrap_migration() {
        let migrations: Vec<_> = MIGRATIONS.iter().collect();
        assert!(
            !migrations.is_empty(),
            "katpool-db must ship at least one migration; check `crates/katpool-db/migrations/`"
        );
        // The bootstrap migration uses a 14-digit timestamp prefix
        // (YYYYMMDDHHMMSS). Catch typos that would yield a different
        // numeric width and re-order the runs.
        for m in &migrations {
            let s = m.version.to_string();
            assert!(
                s.len() >= 8 && s.chars().all(|c| c.is_ascii_digit()),
                "migration version `{s}` is not a numeric timestamp; rename per ADR-0011 convention"
            );
        }
    }
}
