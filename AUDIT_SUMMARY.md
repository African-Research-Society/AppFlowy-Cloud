# AppFlowy-Cloud audit

## Executive Summary

Reviewed the 16 commits ahead of `upstream/main`. The membership gate already refuses a half-configured URL or key. Both variables empty still disables the gate, which is how a stock AppFlowy deploy keeps working. This branch only adds a startup warning for that case.

## Architecture Overview

Rust AppFlowy Cloud. ARS adds a Casbin pre-check that calls the AfriNexus `app_appflowy_access` RPC over HTTPS with a service key, plus Coolify compose and a Supabase auth shim.

## Audit Coverage

ARS delta only (`upstream/main..HEAD`). Upstream Cloud was not re-audited.

## Confirmed Issues

Both membership env vars empty disables the ARS gate with no log. A missed Coolify setting looks like a healthy deploy that does not enforce ARS membership.

## Security Findings

- Gate off when unset: now logged at warn. Not changed to fail startup, because that would break non-ARS use of this fork.
- Operator UUID skips the RPC and still has to pass AppFlowy Owner checks. Intentional.
- Coolify compose defaults `minioadmin` / `password` if env is empty, and nginx exposes `/minio/`. Inherited self-host pattern. Do not ship those defaults.
- Anon JWT in `nginx/nginx.supabase.conf` is the public anon key. Not a signing secret.
- Membership URL has no hostname allowlist. The operator sets it. Misconfiguration can send the service key to the wrong HTTPS host. Not an end-user SSRF.

## Bugs

Healthcheck hits `/health`, which returns `OK` without checking Postgres. That was an intentional correction of a path that did not exist. It is liveness, not readiness.

## Compatibility Findings

None beyond the optional gate.

## Dead/Vestigial Code

Unused CORS map in `nginx/nginx.supabase.conf`. Not removed; it is not applied.

## Mapping/Consistency Problems

None confirmed in the delta.

## Performance/Reliability

Five-second timeout and no redirects on the membership client. Reasonable.

## Testing Gaps

No new Rust test. `cargo check` was not run in this pass.

## Improvements

Startup warning when the gate is off.

## Fixes Implemented

`tracing::warn!` in `ArsMembership::from_env` when both variables are empty.

## Tests Added

None.

## Verification Performed

Code read against the 16-commit diff. Not compiled.

## Findings Not Fixed

Compose default passwords, public MinIO locations, shallow healthcheck, hostname allowlist.

## Items Requiring Human Decision

Set `ARS_MEMBERSHIP_RPC_URL` and `ARS_MEMBERSHIP_SERVICE_KEY` in Coolify before members use mapped workspaces. Decide whether an empty pair should refuse to boot in the ARS deployment only.

## Recommended Future Work

Fail startup when an explicit `ARS_REQUIRE_MEMBERSHIP=1` is set and the pair is empty. Replace compose password defaults in the ARS Coolify file only.

## Statistics

- Commits examined: 16.
- Coverage: the ARS delta, not upstream Cloud.
- Fixed: 1 (logging).
- Remaining: compose defaults, healthcheck depth, URL allowlist.
- Dependencies changed: none.
