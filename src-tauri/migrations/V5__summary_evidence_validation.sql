-- JUL-202 : preuves, édition et validation humaine du compte-rendu

ALTER TABLE summaries ADD COLUMN model TEXT;
ALTER TABLE summaries ADD COLUMN validation_state TEXT NOT NULL DEFAULT 'generated'
    CHECK (validation_state IN ('generated', 'edited', 'validated'));
ALTER TABLE summaries ADD COLUMN validated_at TEXT;

ALTER TABLE actions ADD COLUMN item_key TEXT;
ALTER TABLE actions ADD COLUMN sources_json TEXT;
ALTER TABLE actions ADD COLUMN origin TEXT NOT NULL DEFAULT 'generated'
    CHECK (origin IN ('generated', 'edited', 'validated', 'locked'));

CREATE INDEX idx_actions_meeting_item_key ON actions (meeting_id, item_key);

CREATE TABLE summary_revisions (
    id TEXT PRIMARY KEY NOT NULL,
    summary_id TEXT NOT NULL REFERENCES summaries (id) ON DELETE CASCADE,
    meeting_id TEXT NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    validation_state TEXT NOT NULL CHECK (
        validation_state IN ('generated', 'edited', 'validated')
    ),
    model TEXT,
    provider_id TEXT REFERENCES ai_providers (id) ON DELETE SET NULL,
    note TEXT,
    created_at TEXT NOT NULL
);

CREATE INDEX idx_summary_revisions_summary_id ON summary_revisions (summary_id);
CREATE INDEX idx_summary_revisions_meeting_id ON summary_revisions (meeting_id);
