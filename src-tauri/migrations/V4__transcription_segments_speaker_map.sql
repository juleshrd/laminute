-- JUL-200 : segments structurés (diarisation) et cartographie locuteurs

ALTER TABLE transcriptions ADD COLUMN segments_json TEXT;

ALTER TABLE meetings ADD COLUMN speaker_map_json TEXT;
