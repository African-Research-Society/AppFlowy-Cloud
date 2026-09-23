-- Shadow auth schema for running AppFlowy-Cloud against an external
-- (hosted Supabase) auth server, without the bundled gotrue container.
--
-- Fork-local file, mounted into the postgres container's
-- /docker-entrypoint-initdb.d/ by docker-compose.override.yml. It runs once,
-- on first initialisation of a fresh data volume, BEFORE AppFlowy-Cloud's
-- embedded sqlx migrations.
--
-- Why: upstream migration 20231130150001_user_id_foreign_key.sql adds
--   FOREIGN KEY (af_user.uuid) REFERENCES auth.users (id) ON DELETE CASCADE
-- and re-raises if the auth schema is missing. Normally gotrue creates that
-- schema in this database; with auth delegated to hosted Supabase it never
-- exists. Pre-creating a minimal shadow auth.users lets the upstream
-- migration run verbatim (no tracked-file patch, no fork-built images).
--
-- A BEFORE INSERT trigger on public.af_user keeps the shadow table in step:
-- every provisioned user gets a shadow row, so the FK is always satisfied.
-- af_user does not exist yet when this script runs (AppFlowy's migrations
-- create it at first app startup), so the trigger is installed by a one-shot
-- DDL event trigger that waits for the table to appear.
--
-- Bonus: the FK's ON DELETE CASCADE makes the shadow table an erasure hook —
-- deleting a user's row from auth.users here cascades their af_user row.

CREATE SCHEMA IF NOT EXISTS auth;

-- The column set is not arbitrary: AppFlowy both migrates and queries against
-- gotrue's auth.users directly. Migration 20251123132148_system_admin.sql does
--
--   SELECT id FROM auth.users WHERE deleted_at IS NULL ORDER BY created_at ASC
--
-- and the server joins af_user to auth.users at runtime for au.email,
-- au.raw_app_meta_data, au.is_super_admin and au.updated_at. Anything missing
-- surfaces as a migration abort ('column "deleted_at" does not exist') or a
-- failing query later, so this mirrors the subset gotrue exposes and AppFlowy
-- actually reads. Re-check it after upstream bumps: the published image can
-- carry migrations that are not in the public repository.
CREATE TABLE IF NOT EXISTS auth.users (
  id                uuid PRIMARY KEY,
  email             text,
  deleted_at        timestamptz,
  created_at        timestamptz DEFAULT now(),
  updated_at        timestamptz DEFAULT now(),
  raw_app_meta_data jsonb DEFAULT '{}'::jsonb,
  is_super_admin    boolean DEFAULT false
);

-- Nobody is flagged as a system admin here: that path expects the admin
-- console, which this deployment does not run (doc/SUPABASE_AUTH.md), and
-- AppFlowy refuses client-app sign-in for such accounts. The backfill
-- migration finds the table empty at migration time and skips itself.
CREATE OR REPLACE FUNCTION public.af_user_auth_shadow_fn() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO auth.users (id, email, created_at, updated_at)
  VALUES (NEW.uuid, NEW.email, now(), now())
  ON CONFLICT (id) DO UPDATE
    SET email = EXCLUDED.email,
        updated_at = now();
  RETURN NEW;
END $$;

-- Every reference to public.af_user here must go through a *variable* holding
-- its oid. A literal 'public.af_user'::regclass is constant-folded when the
-- statement is planned, so it raises "relation public.af_user does not exist"
-- even when guarded by to_regclass(...) IS NOT NULL in the same expression —
-- and this trigger runs on the first CREATE TABLE of AppFlowy's migrations,
-- long before af_user exists. That aborts the migration and the server with
-- it. Same reason the CREATE TRIGGER goes through EXECUTE.
CREATE OR REPLACE FUNCTION public.install_af_user_shadow_trigger() RETURNS event_trigger
LANGUAGE plpgsql AS $$
DECLARE
  af_user oid := to_regclass('public.af_user');
BEGIN
  IF af_user IS NULL THEN
    RETURN;
  END IF;

  IF NOT EXISTS (
    SELECT 1 FROM pg_trigger
    WHERE tgname = 'af_user_auth_shadow' AND tgrelid = af_user
  ) THEN
    EXECUTE 'CREATE TRIGGER af_user_auth_shadow'
      || ' BEFORE INSERT OR UPDATE OF uuid, email ON public.af_user'
      || ' FOR EACH ROW EXECUTE FUNCTION public.af_user_auth_shadow_fn()';
  END IF;
END $$;

CREATE EVENT TRIGGER af_user_shadow_installer ON ddl_command_end
  WHEN TAG IN ('CREATE TABLE')
  EXECUTE FUNCTION public.install_af_user_shadow_trigger();
