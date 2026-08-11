-- Persistance minimale des jobs IA (JUL-195)

CREATE TABLE ai_jobs (
    job_id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('transcription', 'summary')),
    meeting_id TEXT REFERENCES meetings (id) ON DELETE CASCADE,
    audio_file_id TEXT REFERENCES audio_files (id) ON DELETE SET NULL,
    phase TEXT NOT NULL,
    status TEXT NOT NULL CHECK (
        status IN ('running', 'completed', 'failed', 'cancelled')
    ),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_ai_jobs_meeting_id ON ai_jobs (meeting_id);
CREATE INDEX idx_ai_jobs_status ON ai_jobs (status);
