-- Index FTS5 trigram pour la recherche historique (JUL-172)

CREATE VIRTUAL TABLE meetings_fts USING fts5(
    meeting_id UNINDEXED,
    source UNINDEXED,
    body,
    tokenize = 'trigram'
);

INSERT INTO meetings_fts (meeting_id, source, body)
SELECT id, 'title', title FROM meetings;

INSERT INTO meetings_fts (meeting_id, source, body)
SELECT meeting_id, 'transcription', content FROM transcriptions;

INSERT INTO meetings_fts (meeting_id, source, body)
SELECT meeting_id, 'summary', content FROM summaries;

CREATE TRIGGER meetings_fts_ai AFTER INSERT ON meetings BEGIN
    INSERT INTO meetings_fts (meeting_id, source, body)
    VALUES (NEW.id, 'title', NEW.title);
END;

CREATE TRIGGER meetings_fts_au AFTER UPDATE OF title ON meetings BEGIN
    DELETE FROM meetings_fts WHERE meeting_id = NEW.id AND source = 'title';
    INSERT INTO meetings_fts (meeting_id, source, body)
    VALUES (NEW.id, 'title', NEW.title);
END;

CREATE TRIGGER meetings_fts_ad AFTER DELETE ON meetings BEGIN
    DELETE FROM meetings_fts WHERE meeting_id = OLD.id;
END;

CREATE TRIGGER transcriptions_fts_ai AFTER INSERT ON transcriptions BEGIN
    INSERT INTO meetings_fts (meeting_id, source, body)
    VALUES (NEW.meeting_id, 'transcription', NEW.content);
END;

CREATE TRIGGER transcriptions_fts_au AFTER UPDATE OF content ON transcriptions BEGIN
    DELETE FROM meetings_fts
    WHERE meeting_id = NEW.meeting_id AND source = 'transcription' AND body = OLD.content;
    INSERT INTO meetings_fts (meeting_id, source, body)
    VALUES (NEW.meeting_id, 'transcription', NEW.content);
END;

CREATE TRIGGER transcriptions_fts_ad AFTER DELETE ON transcriptions BEGIN
    DELETE FROM meetings_fts
    WHERE meeting_id = OLD.meeting_id AND source = 'transcription' AND body = OLD.content;
END;

CREATE TRIGGER summaries_fts_ai AFTER INSERT ON summaries BEGIN
    INSERT INTO meetings_fts (meeting_id, source, body)
    VALUES (NEW.meeting_id, 'summary', NEW.content);
END;

CREATE TRIGGER summaries_fts_au AFTER UPDATE OF content ON summaries BEGIN
    DELETE FROM meetings_fts
    WHERE meeting_id = NEW.meeting_id AND source = 'summary' AND body = OLD.content;
    INSERT INTO meetings_fts (meeting_id, source, body)
    VALUES (NEW.meeting_id, 'summary', NEW.content);
END;

CREATE TRIGGER summaries_fts_ad AFTER DELETE ON summaries BEGIN
    DELETE FROM meetings_fts
    WHERE meeting_id = OLD.meeting_id AND source = 'summary' AND body = OLD.content;
END;
