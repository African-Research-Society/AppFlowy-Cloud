# AppFlowy-Cloud audit (second pass)

## Coverage Matrix

| Subsystem | Depth | Notes |
| --- | --- | --- |
| Access control and ARS membership gate | Adversarial | `config.rs` default, NoOps vs Casbin, `ars_membership.rs` |
| User verify and JWT | Deep | `src/api/user.rs`, `gotrue_jwt.rs`, `authentication/jwt.rs` |
| Blob GET/metadata | Deep | `src/api/file_storage.rs` handlers the web client calls |
| WebSocket token | Deep | `src/api/ws.rs` query token |
| nginx and Coolify compose | Deep | Ports, MinIO, Postgres defaults, anon key |
| Health endpoint | Light | Returns OK; not a readiness probe |
| Collab CRDT, AI, admin UI | Not applicable | Not on the ARS auth or file path we deploy for members |

The client side of these calls was confirmed in the AppFlowy repo (`client-api` http.rs, ws v2, file storage).

## Findings

- If `APPFLOWY_ACCESS_CONTROL` is false (the code default), enforcement is allow-all and the ARS membership gate is not consulted. Critical if a deploy leaves it unset. `deploy.env` in the repo sets it true. Not changed: forcing it on without Casbin data would lock the workspace.
- Blob GET and blob metadata do not require a user. Knowing workspace id and file id is enough. High. Not changed: the desktop and web clients depend on this URL shape. Requiring auth is a cross-repo protocol change.
- `GET /api/user/verify/{access_token}` puts the JWT in the path. High. Same constraint: the pinned client builds that URL.
- WebSocket v2 puts the token in the query string. Desktop currently uses v1 with an Authorization header. Medium.
- Coolify compose defaults `minioadmin` and Postgres `password` if env is empty, and nginx publishes `/minio/`. High if those defaults are left in place. Not changed: would break an existing volume that still uses them.
- Shared JWT secret can mint `service_role` for the same Supabase project. High on secret leak. Architectural, documented in `doc/SUPABASE_AUTH.md`.
- Both ARS membership env vars empty disables the gate. This branch already logs a warning.

## Fixed Findings

Startup warning when the membership gate is off (previous commit on this branch).

## Unfixed Findings

Access-control default, blob GET, verify-token path, MinIO defaults, service-role JWT. All need a coordinated client change or a confirmed production env review.

## Security

See findings. Anon key in nginx is public by design.

## Database Integrity

Not re-audited beyond the auth user table read in the membership gate.

## Authentication

Supabase JWT is the session. AppFlowy does not check `role` or `aud` beyond HS256.

## Authorization

Effective only when `APPFLOWY_ACCESS_CONTROL=true` and the ARS env pair is set.

## Bugs

Healthcheck is liveness only.

## Race Conditions

Not examined in collab.

## Vestigial Code

Unused CORS map in nginx. Not removed.

## Mapping/Consistency Problems

Client sends Authorization on blob GET; server ignores it.

## Compatibility

Do not change verify or blob routes without updating the AppFlowy and AppFlowy-Web pins together.

## Dependencies

Not an advisory scan.

## Performance

Not examined.

## Accessibility

Not applicable to this service.

## Testing

`cargo check` was not run.

## Cross-Repository Findings

AfriNexus `/workspace` and the web client both end in this verify URL. Referrer policy on the AfriNexus side is fixed in that repo. The token still appears in Cloud access logs.

## Product Decisions Required

Turn access control on in every real deploy and fail boot if it is off. Move verify token to a header. Require auth on blob GET. Remove MinIO public console.

## Remaining Risks

A deploy that copies compose without filling env vars.

## Areas Where Audit Confidence Is Low

Whether the live Coolify stack sets `APPFLOWY_ACCESS_CONTROL` and the membership pair. The repo’s `deploy.env` says yes. The running host was not inspected.

## Verification

Read the handlers and the client callers. Not compiled. Not deployed.

## Metrics

- Cloud API areas opened: user verify, file storage GET, ws, access-control, nginx, compose.
- Upstream subsystems excluded with a reason: CRDT, AI, admin UI.
- Findings fixed in code: 1 (log). Protocol fixes: 0.
