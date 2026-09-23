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
