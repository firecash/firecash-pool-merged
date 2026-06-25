-- Least-privilege READ-ONLY Postgres role for the katpool public HTTP API
-- (ADR-0021 DoS/abuse posture; Phase 7/8 hardening).
--
-- The unified `katpool` binary embeds the accountant + payout engines (which
-- WRITE) and the read-only API (which only reads) in one process; by default
-- they share one Postgres connection. Setting KATPOOL_API_DATABASE_URL points
-- the API at a separate connection authenticated as the role created here, so a
-- bug or compromise in the public read surface cannot write or escalate.
--
-- Run ONCE per environment, out-of-band, as the role that OWNS the katpool
-- tables / runs migrations (so the default-privileges grant also covers tables
-- created by future migrations):
--
--     psql "$KATPOOL_DATABASE_URL" -f ops/db/api-readonly-role.sql
--
-- Idempotent: safe to re-run (e.g. after a migration adds tables).

-- 1. Read-only group role: no login, no password, no write privileges.
DO $$
BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'katpool_readonly') THEN
    CREATE ROLE katpool_readonly NOLOGIN;
  END IF;
END $$;

-- 2. Connect + read the public schema (every current table) and nothing else.
DO $$
BEGIN
  EXECUTE format('GRANT CONNECT ON DATABASE %I TO katpool_readonly', current_database());
END $$;
GRANT USAGE ON SCHEMA public TO katpool_readonly;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO katpool_readonly;

-- Future tables created by the role running this script (the migration owner)
-- are auto-granted SELECT, so new migrations need no manual follow-up.
ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO katpool_readonly;

-- 3. OPERATOR STEP (out-of-band; never commit the password). Create the LOGIN
--    user the API connects as, place it in the read-only group, and put its
--    credentials ONLY in KATPOOL_API_DATABASE_URL:
--
--      CREATE ROLE katpool_api LOGIN PASSWORD '<generate-a-strong-secret>';
--      GRANT katpool_readonly TO katpool_api;
--
--      KATPOOL_API_DATABASE_URL=postgres://katpool_api:<secret>@<host>:<port>/<db>
--
--    Leave KATPOOL_API_DATABASE_URL unset to have the API share the writers'
--    pool (dev / single-role deployments).
