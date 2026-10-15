-- Admin-only holding area for reviewed ZIP imports. These are not AppFlowy
-- pages, are never indexed by Kora, and confer no audience grant. Promotion
-- creates canonical AppFlowy document collabs only after Wiki ACL is complete.
CREATE TABLE IF NOT EXISTS ars_wiki_seed_bundle (
    archive_sha256 TEXT PRIMARY KEY CHECK (archive_sha256 ~ '^[0-9a-f]{64}$'),
    archive_name TEXT NOT NULL,
    staged_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    entry_count INTEGER NOT NULL CHECK (entry_count >= 0)
);

CREATE TABLE IF NOT EXISTS ars_wiki_seed_entry (
    archive_sha256 TEXT NOT NULL REFERENCES ars_wiki_seed_bundle(archive_sha256) ON DELETE RESTRICT,
    source_path TEXT NOT NULL,
    source_sha256 TEXT NOT NULL CHECK (source_sha256 ~ '^[0-9a-f]{64}$'),
    body_sha256 TEXT NOT NULL CHECK (body_sha256 ~ '^[0-9a-f]{64}$'),
    metadata JSONB NOT NULL,
    source_markdown TEXT NOT NULL,
    review_issues JSONB NOT NULL DEFAULT '[]'::jsonb,
    staged_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (archive_sha256, source_path),
    CHECK (source_path LIKE 'knowledge/%.md')
);

REVOKE ALL ON ars_wiki_seed_bundle, ars_wiki_seed_entry FROM PUBLIC;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
        REVOKE ALL ON ars_wiki_seed_bundle, ars_wiki_seed_entry FROM anon;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
        REVOKE ALL ON ars_wiki_seed_bundle, ars_wiki_seed_entry FROM authenticated;
    END IF;
END;
$$;
