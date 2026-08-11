import { useCallback, useEffect } from "react";

import {
  displaySpeakerLabel,
  type SpeakerMap,
  uniqueSpeakersFromSegments,
} from "../../lib/speakerMap";
import type { TranscriptionSegment } from "../../lib/transcription";

interface SpeakerMapEditorProps {
  segments?: TranscriptionSegment[];
  speakerMap: SpeakerMap;
  disabled?: boolean;
  onChange: (next: SpeakerMap) => void;
}

export function SpeakerMapEditor({
  segments,
  speakerMap,
  disabled = false,
  onChange,
}: SpeakerMapEditorProps) {
  const speakers = uniqueSpeakersFromSegments(segments);

  const updateName = useCallback(
    (speakerId: string, name: string) => {
      onChange({
        ...speakerMap,
        [speakerId]: name,
      });
    },
    [onChange, speakerMap],
  );

  useEffect(() => {
    if (speakers.length === 0) {
      return;
    }
    const missing = speakers.some((id) => speakerMap[id] === undefined);
    if (!missing) {
      return;
    }
    const next = { ...speakerMap };
    for (const id of speakers) {
      if (next[id] === undefined) {
        next[id] = "";
      }
    }
    onChange(next);
  }, [speakers, speakerMap, onChange]);

  if (speakers.length === 0) {
    return null;
  }

  return (
    <div className="speaker-map-editor">
      <p className="lm-kicker">Locuteurs</p>
      <p className="lm-subtle">
        Associez les labels techniques aux noms des participants. Le texte source n&apos;est pas
        modifié ; les noms confirmés sont utilisés pour le compte-rendu.
      </p>
      <ul className="speaker-map-editor__list">
        {speakers.map((speakerId) => (
          <li key={speakerId}>
            <label htmlFor={`speaker-map-${speakerId}`}>{speakerId}</label>
            <input
              id={`speaker-map-${speakerId}`}
              type="text"
              value={speakerMap[speakerId] ?? ""}
              placeholder={displaySpeakerLabel(speakerId, speakerMap)}
              disabled={disabled}
              onChange={(event) => updateName(speakerId, event.target.value)}
            />
          </li>
        ))}
      </ul>
    </div>
  );
}
