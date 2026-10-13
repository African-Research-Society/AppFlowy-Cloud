# ARS authentication deployment

The ARS web fork uses Better Auth authorization-code flow with PKCE. The ARS
application validates the identity token and live session, then issues a short-lived
AppFlowy-specific HS256 token. Existing UUID-to-AppFlowy-user mappings remain the
identity boundary; do not create replacement users by matching email addresses.
Native desktop/mobile login is outside this deployment's scope.

Deploy the reviewed web and cloud revisions together with Afrinexus. Set:

- `APPFLOWY_WEB_REVISION`: the reviewed commit containing the ARS PKCE flow.
- `ARS_AUTH_ORIGIN`: the HTTPS Afrinexus origin (use the isolated origin during rehearsal).
- `ARS_AUTH_API_URL`: `https://africanresearchsociety.org/api/internal/appflowy`.
- `ARS_AUTH_ISSUER`: `https://africanresearchsociety.org/api/integrations/appflowy`.
- `ARS_INTERNAL_API_URL`: `https://africanresearchsociety.org/api/internal/integrations`.
- `ARS_INTERNAL_API_KEY`: the shared server-only internal API credential.
- `APPFLOWY_BRIDGE_SECRET`: the shared server-only compatibility signing secret;
  use a fresh secret distinct from Better Auth's key and old GoTrue keys.
- `ARS_WORKSPACE_OPERATOR_UUID`: the preserved, verified ARS operator identity,
  if automated workspace administration is enabled.

Register the exact AppFlowy HTTPS callback origin using Afrinexus's
`register-appflowy-client.mjs`. The public client is `ars-appflowy-web`, requires
PKCE, and receives session-bound identity tokens. Google and email credentials
are handled only by the ARS application. `/gotrue` on the AppFlowy ingress returns
410; direct account deletion is disabled in favor of the ARS audited erasure flow.

Cloud validates the bridge issuer, audience, expiry and session identifier, then
checks the active ARS identity on authenticated requests. WebSocket connections
also recheck the ARS session every ten seconds and close on failure. A revoked or
expired session therefore requires signing in again; loss of the identity service
fails closed. AI/search compatibility tokens are short-lived and expire no later
than the underlying ARS session. Do not expose signing secrets to the web build.

`docker-compose.coolify.yml` is generated from the base and ARS override with
`python3 script/gen_coolify_compose.py`; do not edit it independently. The local
PostgreSQL `auth` compatibility schema is retained for upstream migrations. It is
not a running Supabase service. Keep `postgres_auth_shim_data` and every existing
AppFlowy object volume attached exactly as before. Do not attach the new ARS
application database or object volumes in their place.

Before production, rehearse against restored copies and verify workspace ownership,
invitation acceptance, membership denial, editing, token refresh/reconnect, and
stream revocation. Stop Cloud workers and writes during the final data transfer.
Verify the whole coordinated stack with Supabase destinations blocked before
reopening writes. See Afrinexus's database runbook for retention and rollback gates.

For the ARS ARM server, build the locked source natively with Rust 1.86 and the
`ci` profile (optimized, without release LTO). The compiler image is
`rust:1.86-bookworm@sha256:300ec56abce8cc9448ddea2172747d048ed902a3090e6b57babb2bf19f754081`.
Install `protobuf-compiler`, `libprotobuf-dev`, `clang` and `lld`, then run
`SQLX_OFFLINE=true CARGO_BUILD_JOBS=2 cargo build --locked --profile ci --bin appflowy_cloud`.
Mount compiler caches and target output on the separate build disk and limit the
compiler container to two CPUs and 7 GiB. Use a Git archive of the reviewed source;
never mount deployment env files into the compiler.

Copy only the compiled `appflowy_cloud` binary into an empty artifact directory.
Build `docker/ars-runtime/Dockerfile` with that directory as its context and set
`SOURCE_REVISION` to the exact source commit. Build Web with the full reviewed
`APPFLOWY_WEB_REVISION` SHA, then build the ARS Nginx image. Preserve the original
images in the encrypted off-server rollback archive before replacing them.

`gen_coolify_compose.py` also writes `docker-compose.ars.yml` for Coolify's raw
Compose mode. It uses reviewed, prebuilt image references, private database/object
networking and explicit per-service credentials. Set the four `ARS_APPFLOWY_*_IMAGE`
variables, `APPFLOWY_INGRESS_HOST`, the existing TLS file paths and the exact
`APPFLOWY_POSTGRES_VOLUME` / `APPFLOWY_MINIO_VOLUME` names. Existing production
volumes are `gwyoyg55y0vtcys6qrgrnj7a_postgres-auth-shim-data` and
`gwyoyg55y0vtcys6qrgrnj7a_minio-data`. Rehearsal must use separate restored volumes.
Keep raw Compose enabled; normal parsing would distribute unrelated secrets to
every service. Cached MinIO is deliberately never pulled; restore its archived
image on a new host.
