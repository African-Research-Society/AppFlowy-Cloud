-- Canonical wiki bodies remain AppFlowy document collabs. This metadata is
-- intentionally dormant until every Cloud read/write surface enforces audience.
CREATE TABLE ars_wiki_page (
    page_id UUID PRIMARY KEY,
    workspace_id UUID NOT NULL REFERENCES af_workspace(workspace_id) ON DELETE CASCADE,
    owner_uuid UUID NOT NULL REFERENCES af_user(uuid),
    audience_kind TEXT NOT NULL CHECK (audience_kind IN
        ('ars_committee', 'chapter_committee', 'chapter_executive', 'named_user')),
    chapter_id UUID,
    named_user_uuid UUID,
    guidance_kind TEXT NOT NULL CHECK (guidance_kind IN ('wiki', 'sop')),
    review_due_at TIMESTAMPTZ,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((audience_kind IN ('chapter_committee', 'chapter_executive')) = (chapter_id IS NOT NULL)),
    CHECK ((audience_kind = 'named_user') = (named_user_uuid IS NOT NULL))
);
CREATE INDEX ars_wiki_page_audience ON ars_wiki_page(audience_kind, chapter_id)
    WHERE archived_at IS NULL;

CREATE OR REPLACE FUNCTION ars_wiki_require_access_enforcement()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Restricted wikis are disabled until Cloud enforces audience on all content and metadata APIs';
END;
$$;
CREATE TRIGGER ars_wiki_access_gate BEFORE INSERT ON ars_wiki_page
FOR EACH ROW EXECUTE FUNCTION ars_wiki_require_access_enforcement();

-- A restricted wiki must never enter AppFlowy's public publishing store,
-- including through legacy bulk-publish endpoints or a direct DB writer.
CREATE OR REPLACE FUNCTION ars_wiki_block_publication()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF EXISTS (SELECT 1 FROM ars_wiki_page WHERE page_id = NEW.view_id) THEN
        RAISE EXCEPTION 'Restricted wiki pages cannot be published publicly';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER ars_wiki_no_publication BEFORE INSERT OR UPDATE ON af_published_collab
FOR EACH ROW EXECUTE FUNCTION ars_wiki_block_publication();

-- A live AppFlowy edit is provisional. A separate immutable snapshot records
-- what the owner and administrator actually verified.
CREATE TABLE ars_wiki_verification (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    page_id UUID NOT NULL REFERENCES ars_wiki_page(page_id) ON DELETE RESTRICT,
    snapshot_id BIGINT NOT NULL REFERENCES af_collab_snapshot(sid) ON DELETE RESTRICT,
    revision_hash TEXT NOT NULL CHECK (revision_hash ~ '^[0-9a-f]{64}$'),
    owner_uuid UUID NOT NULL,
    owner_signed_at TIMESTAMPTZ,
    admin_uuid UUID NOT NULL,
    admin_approved_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    override_reason TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (page_id, revision_hash),
    CHECK (owner_signed_at IS NOT NULL OR
        (override_reason IS NOT NULL AND length(trim(override_reason)) >= 20))
);
CREATE INDEX ars_wiki_verification_latest ON ars_wiki_verification(page_id, admin_approved_at DESC);

CREATE TABLE ars_wiki_proposal (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    page_id UUID REFERENCES ars_wiki_page(page_id) ON DELETE RESTRICT,
    author_uuid UUID NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('new', 'change', 'verify', 'archive', 'rollback', 'reassign')),
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending', 'approved', 'rejected', 'withdrawn')),
    proposed_audience_kind TEXT CHECK (proposed_audience_kind IN
        ('ars_committee', 'chapter_committee', 'chapter_executive', 'named_user')),
    proposed_chapter_id UUID,
    base_revision_hash TEXT,
    proposed_revision_hash TEXT,
    source_path TEXT,
    provenance JSONB NOT NULL DEFAULT '{}'::jsonb,
    rationale TEXT NOT NULL DEFAULT '',
    decided_by UUID,
    decided_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ars_wiki_proposal_queue ON ars_wiki_proposal(state, created_at);

CREATE TABLE ars_wiki_comment (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    proposal_id UUID NOT NULL REFERENCES ars_wiki_proposal(id) ON DELETE RESTRICT,
    author_uuid UUID NOT NULL,
    body TEXT NOT NULL CHECK (length(trim(body)) > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE ars_wiki_audit (
    id BIGSERIAL PRIMARY KEY,
    page_id UUID,
    proposal_id UUID,
    actor_uuid UUID NOT NULL,
    action TEXT NOT NULL,
    detail JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX ars_wiki_audit_page_time ON ars_wiki_audit(page_id, created_at DESC);

CREATE OR REPLACE FUNCTION ars_wiki_forbid_mutation()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'Wiki verification and audit records are immutable';
END;
$$;
CREATE TRIGGER ars_wiki_verification_immutable BEFORE UPDATE OR DELETE ON ars_wiki_verification
FOR EACH ROW EXECUTE FUNCTION ars_wiki_forbid_mutation();
CREATE TRIGGER ars_wiki_audit_immutable BEFORE UPDATE OR DELETE ON ars_wiki_audit
FOR EACH ROW EXECUTE FUNCTION ars_wiki_forbid_mutation();

-- Cloud's application role owns these records. A shared PostgREST instance
-- must not expose titles, proposals, comments, or verification metadata.
REVOKE ALL ON ars_wiki_page, ars_wiki_verification, ars_wiki_proposal,
    ars_wiki_comment, ars_wiki_audit FROM PUBLIC;
REVOKE ALL ON SEQUENCE ars_wiki_audit_id_seq FROM PUBLIC;
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'anon') THEN
        REVOKE ALL ON ars_wiki_page, ars_wiki_verification, ars_wiki_proposal,
            ars_wiki_comment, ars_wiki_audit FROM anon;
        REVOKE ALL ON SEQUENCE ars_wiki_audit_id_seq FROM anon;
    END IF;
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'authenticated') THEN
        REVOKE ALL ON ars_wiki_page, ars_wiki_verification, ars_wiki_proposal,
            ars_wiki_comment, ars_wiki_audit FROM authenticated;
        REVOKE ALL ON SEQUENCE ars_wiki_audit_id_seq FROM authenticated;
    END IF;
END;
$$;
