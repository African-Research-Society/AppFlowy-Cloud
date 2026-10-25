#!/usr/bin/env python3
"""Stage a reviewed Kora seed preview in the local AppFlowy PostgreSQL DB.

This does not create pages or expose staged content through Cloud APIs. The
preview manifest must be generated from the ZIP first. No archive code runs.
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import subprocess
import sys
import zipfile
from pathlib import Path

LOCAL_CONTAINER = "appflowy-cloud-postgres-1"
ZIP_PREFIX = "ars-knowledge-base-v1/"


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def checked_rows(archive: Path, manifest: Path) -> tuple[dict, list[dict]]:
    preview = json.loads(manifest.read_text(encoding="utf-8"))
    overview = preview["overview"]
    if digest(archive.read_bytes()) != overview["archive_sha256"]:
        raise ValueError("ZIP does not match the preview archive hash")
    rows: list[dict] = []
    with zipfile.ZipFile(archive) as packed:
        for page in preview["pages"]:
            if page["disposition"] != "review_before_import":
                continue
            path = page["path"]
            if not path.startswith("knowledge/") or not path.endswith(".md") or ".." in Path(path).parts:
                raise ValueError(f"Unsafe preview path: {path}")
            raw = packed.read(ZIP_PREFIX + path)
            if digest(raw) != page["source_sha256"]:
                raise ValueError(f"Page changed since preview: {path}")
            rows.append({
                "source_path": path,
                "source_sha256": page["source_sha256"],
                "body_sha256": page["body_sha256"],
                "metadata": page,
                "source_markdown": raw.decode("utf-8-sig"),
                "review_issues": page["issues"],
            })
    if len(rows) != overview["content_pages"] or len({r["source_path"] for r in rows}) != len(rows):
        raise ValueError("Preview page count or paths are inconsistent")
    return overview, rows


def stage_sql(overview: dict, rows: list[dict]) -> str:
    # Base64 keeps user-supplied Markdown out of SQL syntax and psql commands.
    payload = base64.b64encode(json.dumps(rows, ensure_ascii=False).encode()).decode()
    archive_name = base64.b64encode(overview["archive_name"].encode()).decode()
    archive_hash = overview["archive_sha256"]
    if len(archive_hash) != 64 or any(c not in "0123456789abcdef" for c in archive_hash):
        raise ValueError("Invalid archive hash")
    return f"""BEGIN;
INSERT INTO ars_wiki_seed_bundle (archive_sha256, archive_name, entry_count)
VALUES ('{archive_hash}', convert_from(decode('{archive_name}', 'base64'), 'UTF8'), {len(rows)})
ON CONFLICT (archive_sha256) DO NOTHING;
INSERT INTO ars_wiki_seed_entry
    (archive_sha256, source_path, source_sha256, body_sha256, metadata, source_markdown, review_issues)
SELECT '{archive_hash}', r.source_path, r.source_sha256, r.body_sha256,
       r.metadata, r.source_markdown, r.review_issues
FROM jsonb_to_recordset(convert_from(decode('{payload}', 'base64'), 'UTF8')::jsonb)
    AS r(source_path text, source_sha256 text, body_sha256 text,
         metadata jsonb, source_markdown text, review_issues jsonb)
ON CONFLICT (archive_sha256, source_path) DO NOTHING;
DO $check$
BEGIN
    IF (SELECT count(*) FROM ars_wiki_seed_entry WHERE archive_sha256 = '{archive_hash}') <> {len(rows)} THEN
        RAISE EXCEPTION 'Staged seed count differs from reviewed preview';
    END IF;
END;
$check$;
COMMIT;
SELECT count(*) AS staged_entries FROM ars_wiki_seed_entry WHERE archive_sha256 = '{archive_hash}';
"""


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("archive", type=Path)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--apply-local", action="store_true", help="Write to the local Docker AppFlowy PostgreSQL only")
    args = parser.parse_args()
    overview, rows = checked_rows(args.archive, args.manifest)
    if not args.apply_local:
        print(f"Validated {len(rows)} pages from {overview['archive_sha256']}; no database write")
        return
    command = ["docker", "exec", "-i", LOCAL_CONTAINER, "psql", "-X", "-v", "ON_ERROR_STOP=1", "-U", "postgres", "-d", "postgres"]
    result = subprocess.run(command, input=stage_sql(overview, rows), text=True, capture_output=True, check=False)
    if result.returncode:
        print(result.stderr.strip() or "Local staging failed", file=sys.stderr)
        raise SystemExit(result.returncode)
    print(result.stdout.strip())


if __name__ == "__main__":
    main()
