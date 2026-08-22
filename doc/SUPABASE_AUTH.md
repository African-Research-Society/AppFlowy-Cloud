# Running AppFlowy-Cloud against hosted Supabase Auth

This fork deployment delegates ALL authentication to the afrinexus Supabase
project (`ars-moya`, ref `ffvwjxcfmtgwogilzupd`). Supabase Auth **is** GoTrue,
so the bundled `gotrue` container is not deployed at all; every GoTrue consumer
— AppFlowy Web, the mobile app, and the Rust server itself — talks to
`https://<ref>.supabase.co/auth/v1/` through an apikey-injecting nginx hop.
Both systems share one `auth.users` pool: same user UUID (`sub`), same email.

**No upstream tracked file is modified.** The integration is three new files
plus env vars, so updating from upstream is a plain merge:

| File | Purpose |
|---|---|
| `nginx/nginx.supabase.conf` | Replaces `nginx/nginx.conf`; proxies `/gotrue/` to Supabase with the `apikey` header injected, plus an internal `:81` listener for the Rust server's own GoTrue calls. Blocks `DELETE /api/user` and public `/gotrue/admin/`. Baked into the nginx image by `docker/supabase-nginx/Dockerfile`. |
| `docker-compose.override.yml` | Disables `gotrue` + `admin_frontend`, swaps the nginx config mount, mounts the DB shim, fixes `appflowy_cloud` dependencies. Auto-merged by `docker compose up` (compose v2.24.4+). |
| `docker/supabase-shim/00-auth-shim.sql` | Postgres initdb script creating a minimal shadow `auth.users` so the upstream FK migration (`migrations/20231130150001_user_id_foreign_key.sql`) runs verbatim; a trigger keeps the shadow table in step with `af_user`. |

## Hard constraint: HS256 must stay the current signing key

AppFlowy-Cloud validates JWTs as **HS256 with a shared secret** (single choke
point: `libs/gotrue-entity/src/gotrue_jwt.rs`). The **legacy HS256 secret must
remain the key that signs new access tokens** (Dashboard → Project Settings →
JWT Keys). Do **not** complete a migration to the ECC key while this
deployment exists, or all AppFlowy auth breaks (the closed-source `ai` /
`appflowy_search` images can only validate HS256 either way).

State as of 2026-08-22 (rotated for this integration):

| Status | Key ID | Type |
|---|---|---|
| **Current** | `BF564DBB-A4ED-415A-8885-44F0DF6FB9A8` | Legacy HS256 (shared secret) |
| Previously used | `9850EDDB-8498-4E8D-811E-6998D10F6386` | ECC P-256 — verify-only, was current until this rotation |
| Revoked | `5F24A356-4650-46D0-BBF5-FC6CEAC8D375` | HS256 (shared secret) |

**Gotcha — only the *legacy* HS256 key's secret is readable.** Supabase
exposes a shared secret only for the legacy key (JWT Keys → *Legacy JWT
Secret* tab). A non-legacy HS256 signing key created in the new signing-keys
UI has no "reveal secret" action anywhere (its ⋮ menu offers only *Move to
previously used* / *Revoke key*), so it can never be used for local
validation — it is useless to AppFlowy, and less usable than the ECC key,
whose public half at least ships in `/auth/v1/.well-known/jwks.json`. That is
why the legacy key was promoted back to current and `5F24A356…` revoked.

Check: decode the first segment of a fresh access token (`base64 -d`) — the
header must read `"alg":"HS256"`.

## One-time Supabase dashboard setup

1. **JWT Keys**: confirm the legacy HS256 key is the *current* key (above).
   Copy its secret from the **Legacy JWT Secret** tab — that value is
   `GOTRUE_JWT_SECRET`. Also grab the anon key and keep service_role handy.
2. **Auth → URL Configuration → Redirect URLs**: add
   - `https://<appflowy-domain>/**` (AppFlowy Web callback)
   - `appflowy-flutter://login-callback` (mobile deep link)

## Deployment configuration

1. In `nginx/nginx.supabase.conf`, set `$supabase_anon_key` (two places, one
   per server block) to the project's anon key. It is public by design and
   safe to commit. `$supabase_host` is already set to the project host.
2. Environment (Coolify env vars, or `.env` from `deploy.env`):
   - `GOTRUE_JWT_SECRET=<Supabase legacy JWT secret>` — the base compose fans
     this out as `APPFLOWY_GOTRUE_JWT_SECRET` to `appflowy_cloud`, `ai`, and
     `appflowy_search`.
   - `APPFLOWY_GOTRUE_BASE_URL=http://nginx:81/gotrue` — the Rust server's
     `user_info` (provisioning), admin calls, and health check go through the
     internal apikey hop. The self-minted service-role token is signed with
     the shared secret and is therefore a valid Supabase admin credential.
   - Everything else (`APPFLOWY_BASE_URL`, S3, database, SMTP for AppFlowy's
     own mailer) as in `deploy.env`. All `GOTRUE_*` container vars are dead —
     magic-link emails now come from Supabase's SMTP and templates.
3. **Fresh Postgres volume required** on first deploy (the initdb shim only
   runs when the data directory is initialised). Deploying onto an existing
   volume instead? The `auth` schema is already there (the bundled gotrue
   created it), so the FK migration is satisfied — run the trigger half of
   `docker/supabase-shim/00-auth-shim.sql` by hand against the live database
   (`CREATE OR REPLACE FUNCTION public.af_user_auth_shadow_fn` + `CREATE
   TRIGGER af_user_auth_shadow`; the event trigger is only needed when
   `af_user` does not exist yet). Note that users keep their old gotrue UUIDs
   in `af_user` while their new Supabase identity is a different `sub`, so
   pre-existing workspaces are not reachable from the new logins.
4. **Coolify** reads exactly one compose file per application, so it never
   picks up `docker-compose.override.yml`. Commit the flattened file and set
   the app's **Docker Compose Location** to `/docker-compose.coolify.yml`.
   Regenerate it after every upstream merge that touches `docker-compose.yml`:

   ```sh
   python3 script/gen_coolify_compose.py     # needs PyYAML + docker compose
   ```

   The generator keeps `${VAR}` references intact (Coolify injects the
   environment), drops the disabled `gotrue` / `admin_frontend` services
   outright rather than trusting Coolify to honour compose profiles, and
   strips the project-name prefix `docker compose config` stamps onto named
   volumes — that prefix would otherwise point the deployment at a different
   (empty) `postgres_data` volume than the one it is already running on.

   The live deployment is app `gwyoyg55y0vtcys6qrgrnj7a` (`appflowy-cloud`)
   on `coolify.dev3.studio`, tracking `main`, with the `nginx` service mapped
   to `appflowy.africanresearchsociety.org` via `NGINX_PORT=18080`.

   **Never add a relative bind mount for a new config file on Coolify.** It
   rewrites them into managed "file storage" entries under
   `/data/coolify/applications/<uuid>/` and creates the host path itself — as
   an empty **directory** when it has no stored content for it. nginx then
   dies at startup (`error mounting ... not a directory`), and postgres
   quietly starts with `/docker-entrypoint-initdb.d/00-auth-shim.sql` as a
   directory, initialising a database with no `auth` schema, which surfaces
   much later as a failed FK migration. Both files therefore travel inside
   images (`docker/supabase-nginx/Dockerfile`,
   `docker/supabase-shim/Dockerfile`). The pre-existing `nginx/ssl/*` mounts
   are fine — Coolify has real content stored for those paths.

## How sign-in works

- **AppFlowy Web**: the login page's Google / magic-link buttons hit
  `{base}/gotrue/...`, which is Supabase — the same provider and user pool as
  afrinexus. First authenticated call to `/api/user/verify/{token}` provisions
  the `af_user` row + starter workspace.
- **Seamless hand-off from afrinexus**: `https://<afrinexus>/workspace` mints
  a fresh single-use session server-side (admin `generateLink` → GoTrue
  `/verify` → redirect to AppFlowy Web `/auth/callback#access_token=...`).
  The user's own afrinexus tokens are never shared (refresh-token rotation
  would revoke the session family).
- **Mobile app**: Settings → Self-hosted → `https://<appflowy-domain>`.
  OAuth opens `{base}/gotrue/authorize?provider=google&redirect_to=appflowy-flutter://login-callback`
  and the deep link returns tokens to the app.

## Verification checklist

```sh
curl https://<appflowy-domain>/gotrue/health        # 200 (401 = anon key not set in nginx conf)
curl https://<appflowy-domain>/gotrue/settings      # Supabase settings JSON, google: true
# throwaway password user created via dashboard:
curl -X POST 'https://<appflowy-domain>/gotrue/token?grant_type=password' \
  -H 'Content-Type: application/json' -d '{"email":"...","password":"..."}'
curl https://<appflowy-domain>/api/user/verify/<access_token>   # {"is_new":true}, then false
curl -X POST 'https://<appflowy-domain>/gotrue/token?grant_type=refresh_token' \
  -H 'Content-Type: application/json' -d '{"refresh_token":"..."}'
curl -X DELETE https://<appflowy-domain>/api/user               # 403 (nginx)
curl https://<appflowy-domain>/gotrue/admin/users               # 403 (nginx)
```

Then: sign in on AppFlowy Web with an afrinexus account (Google + magic
link), open one document in two sessions and type (WS auth), and walk the
`/workspace` hand-off from afrinexus.

Log signatures: `appflowy_cloud` "fail to decode token" = JWT secret mismatch
or the project silently moved to ES256; nginx 401 on `/gotrue/*` = apikey not
reaching Supabase; migration errors at boot = shim did not run (stale volume).

## Deliberate limitations

- **admin_frontend is not deployed.** Its admin gate needs a GoTrue user with
  `role == "supabase_admin"`; granting that on a hosted-Supabase user would
  corrupt PostgREST role mapping for afrinexus. Desktop-app sign-in via its
  login page is therefore unavailable (web + mobile are the supported
  clients).
- **Account deletion from AppFlowy clients is blocked** at nginx. Deletion
  and POPIA erasure are owned by afrinexus. Known gap: afrinexus erasure does
  not yet purge AppFlowy data — the hook is deleting the user's row from this
  deployment's shadow `auth.users` (cascades `af_user`) plus workspace/S3
  cleanup.
- If the Supabase project ever moves to ES256 signing, see the plan addendum:
  `libs/gotrue-entity/src/gotrue_jwt.rs` needs asymmetric-key support (~30
  lines), images must be fork-built, and AI/search token validation breaks.
