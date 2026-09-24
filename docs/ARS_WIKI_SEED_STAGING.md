# ARS Wiki seed staging

The ARS Markdown ZIP is staged in the **running local AppFlowy PostgreSQL
database**, in `ars_wiki_seed_bundle` and `ars_wiki_seed_entry`. These tables
hold the source Markdown, preview metadata, review issues, source path, and
hashes. They are private to the database owner and are not AppFlowy pages or
Kora search sources. Staging does not assign an owner, audience, or verification.

Git contains the table migration and importer, **not the 75 page records**.
The local Docker PostgreSQL volume persists records across service restarts.
A hosted AppFlowy database is a separate environment and has not been changed.

## Local operation

1. Take a database backup and verify it with `pg_restore --list` before writes.
2. Generate and review the manifest with Afrinexus's
   `tools/kora-kb-seed-preview.py`.
3. Apply `migrations/20260924103000_ars_wiki_seed_staging.sql` to the local
   AppFlowy database. It is also part of Cloud's normal future migration set.
4. Run `scripts/ars_wiki_seed_stage.py ZIP MANIFEST --apply-local`.
   Without `--apply-local`, it only checks archive and page hashes. A repeat
   run with the same archive hash inserts no duplicates.
5. Check the staging count and review issues. Never promote the ZIP's
   `sensitivity` label directly into an access grant.

## Promotion gate

Create canonical AppFlowy document collabs only after owners and exact
audiences have been reviewed, Cloud enforces those audiences across content,
titles, attachments, history, export, comments, and direct APIs, and
two-user isolation checks pass. The `ars_wiki_page` migration deliberately
rejects page registration until that enforcement is complete. Promotion
should use an idempotent, audited admin workflow, preserving each source hash.
SOP verification is a separate owner and administrator decision.
