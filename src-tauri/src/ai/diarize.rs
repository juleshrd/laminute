#[derive(Debug, Clone)]
pub struct DiarizedSegment {
    pub speaker: Option<String>,
    pub text: String,
    pub start: Option<f64>,
    pub end: Option<f64>,
}

pub fn format_diarized_text(segments: &[DiarizedSegment], fallback: &str) -> String {
    if segments.is_empty() {
        return fallback.to_string();
    }

    let mut lines = Vec::with_capacity(segments.len());
    for segment in segments {
        let text = segment.text.trim();
        if text.is_empty() {
            continue;
        }
        let speaker = segment
            .speaker
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("Locuteur");
        match (segment.start, segment.end) {
            (Some(start), Some(end)) => {
                lines.push(format!("[{speaker} {start:.1}s–{end:.1}s] {text}"));
            }
            _ => lines.push(format!("[{speaker}] {text}")),
        }
    }

    if lines.is_empty() {
        fallback.to_string()
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_diarized_text_returns_fallback_when_empty() {
        assert_eq!(format_diarized_text(&[], "texte brut"), "texte brut");
    }

    #[test]
    fn format_diarized_text_formats_speakers_and_timestamps() {
        let segments = vec![
            DiarizedSegment {
                speaker: Some("A".to_string()),
                text: "Bonjour.".to_string(),
                start: Some(0.0),
                end: Some(1.0),
            },
            DiarizedSegment {
                speaker: Some("B".to_string()),
                text: "Salut.".to_string(),
                start: Some(1.2),
                end: Some(2.0),
            },
        ];
        let text = format_diarized_text(&segments, "fallback");
        assert!(text.contains("[A 0.0s–1.0s] Bonjour."));
        assert!(text.contains("[B 1.2s–2.0s] Salut."));
    }

    #[test]
    fn format_diarized_text_uses_default_speaker_without_id() {
        let segments = vec![DiarizedSegment {
            speaker: None,
            text: "Hello".to_string(),
            start: None,
            end: None,
        }];
        let text = format_diarized_text(&segments, "fallback");
        assert_eq!(text, "[Locuteur] Hello");
    }
}
