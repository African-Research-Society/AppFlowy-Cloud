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
| `nginx/nginx.supabase.conf` | Replaces `nginx/nginx.conf` at runtime; proxies `/gotrue/` to Supabase with the `apikey` header injected, plus an internal `:81` listener for the Rust server's own GoTrue calls. Blocks `DELETE /api/user` and public `/gotrue/admin/`. |
| `docker-compose.override.yml` | Disables `gotrue` + `admin_frontend`, swaps the nginx config mount, mounts the DB shim, fixes `appflowy_cloud` dependencies. Auto-merged by `docker compose up` (compose v2.24.4+). |
| `docker/supabase-shim/00-auth-shim.sql` | Postgres initdb script creating a minimal shadow `auth.users` so the upstream FK migration (`migrations/20231130150001_user_id_foreign_key.sql`) runs verbatim; a trigger keeps the shadow table in step with `af_user`. |

## Hard constraint: HS256 must stay the current signing key

AppFlowy-Cloud validates JWTs as **HS256 with a shared secret** (single choke
point: `libs/gotrue-entity/src/gotrue_jwt.rs`). The Supabase project has both
an ECC (P-256) key and the legacy HS256 shared secret — the **legacy HS256
secret must remain the key that signs new access tokens** (Dashboard →
Project Settings → JWT Keys). Do **not** complete a migration to the ECC key
while this deployment exists, or all AppFlowy auth breaks (the closed-source
`ai` / `appflowy_search` images can only validate HS256 either way).

Check: decode the first segment of a fresh access token (`base64 -d`) — the
header must read `"alg":"HS256"`.

## One-time Supabase dashboard setup

1. **JWT Keys**: confirm the legacy HS256 secret is current (above). Copy the
   legacy JWT secret, the anon key, and keep the service_role key handy.
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
   runs when the data directory is initialised).
4. Coolify: if its compose invocation does not auto-merge
   `docker-compose.override.yml`, generate a merged file and point Coolify at
   it (regenerate after each upstream merge):

   ```sh
   docker compose -f docker-compose.yml -f docker-compose.override.yml config > docker-compose.coolify.yml
   ```

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
