-- Schéma initial La Minute (JUL-150)
-- Les clés API ne sont JAMAIS stockées ici : credential_key_id pointe vers le trousseau OS.

CREATE TABLE settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE ai_providers (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    provider_type TEXT NOT NULL,
    base_url TEXT,
    model_default TEXT,
    is_enabled INTEGER NOT NULL DEFAULT 1 CHECK (is_enabled IN (0, 1)),
    credential_key_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE meetings (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    description TEXT,
    status TEXT NOT NULL DEFAULT 'draft' CHECK (
        status IN ('draft', 'recording', 'processing', 'completed')
    ),
    started_at TEXT,
    ended_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE audio_files (
    id TEXT PRIMARY KEY NOT NULL,
    meeting_id TEXT NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    file_path TEXT NOT NULL,
    duration_ms INTEGER,
    format TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE transcriptions (
    id TEXT PRIMARY KEY NOT NULL,
    meeting_id TEXT NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    audio_file_id TEXT REFERENCES audio_files (id) ON DELETE SET NULL,
    provider_id TEXT REFERENCES ai_providers (id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    language TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE summaries (
    id TEXT PRIMARY KEY NOT NULL,
    meeting_id TEXT NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    provider_id TEXT REFERENCES ai_providers (id) ON DELETE SET NULL,
    content TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE actions (
    id TEXT PRIMARY KEY NOT NULL,
    meeting_id TEXT NOT NULL REFERENCES meetings (id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    description TEXT,
    assignee TEXT,
    due_date TEXT,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (
        status IN ('pending', 'in_progress', 'done', 'cancelled')
    ),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_audio_files_meeting_id ON audio_files (meeting_id);
CREATE INDEX idx_transcriptions_meeting_id ON transcriptions (meeting_id);
CREATE INDEX idx_summaries_meeting_id ON summaries (meeting_id);
CREATE INDEX idx_actions_meeting_id ON actions (meeting_id);
