//! Estimation tokens, découpage et fusion pour le pipeline map-reduce (JUL-198).

use crate::ai::error::AiError;
use crate::ai::structured_summary::{StructuredActionItem, StructuredSummary};

/// Approximation chars → tokens (MVP, pas tiktoken).
pub const CHARS_PER_TOKEN: usize = 4;

/// Marge réservée à la sortie modèle (≥ 20 %).
pub const OUTPUT_MARGIN_RATIO: f64 = 0.20;

/// Segments de chevauchement entre chunks consécutifs.
pub const DEFAULT_OVERLAP_SEGMENTS: usize = 2;

/// Tokens fixes réservés au prompt système + gabarit utilisateur (hors transcription).
pub const PROMPT_OVERHEAD_TOKENS: usize = 800;

/// Fenêtre de contexte par défaut si le modèle est inconnu.
pub const DEFAULT_CONTEXT_WINDOW_TOKENS: usize = 8_192;

/// Tarifs cloud approximatifs documentés (USD / million de tokens, entrée / sortie).
/// Source indicative — à ajuster si les grilles fournisseurs changent.
const MISTRAL_SMALL_INPUT_USD_PER_M: f64 = 0.10;
const MISTRAL_SMALL_OUTPUT_USD_PER_M: f64 = 0.30;
const MISTRAL_MEDIUM_INPUT_USD_PER_M: f64 = 0.40;
const MISTRAL_MEDIUM_OUTPUT_USD_PER_M: f64 = 2.00;
const OPENAI_GPT4O_MINI_INPUT_USD_PER_M: f64 = 0.15;
const OPENAI_GPT4O_MINI_OUTPUT_USD_PER_M: f64 = 0.60;
const OPENAI_GPT4O_INPUT_USD_PER_M: f64 = 2.50;
const OPENAI_GPT4O_OUTPUT_USD_PER_M: f64 = 10.00;
const OLLAMA_LOCAL_USD_PER_M: f64 = 0.0;

/// Estime le nombre de tokens d'un texte (chars / 4, arrondi supérieur).
pub fn estimate_tokens(text: &str) -> usize {
    let chars = text.chars().count();
    chars.div_ceil(CHARS_PER_TOKEN).max(1)
}

/// Fenêtre de contexte effective pour un modèle de compte-rendu.
pub fn model_context_window_tokens(provider_id: &str, model: Option<&str>) -> usize {
    let model = model.unwrap_or("");
    match provider_id {
        "mistral" => match model {
            m if m.contains("medium") => 32_768,
            _ => 32_768,
        },
        "openai" => match model {
            m if m.contains("gpt-4.1") => 1_047_576,
            m if m.contains("gpt-4o") => 128_000,
            _ => 128_000,
        },
        "ollama" => 8_192,
        _ => DEFAULT_CONTEXT_WINDOW_TOKENS,
    }
}

/// Budget tokens disponibles pour la transcription dans un appel unique.
pub fn effective_input_token_budget(
    provider_id: &str,
    model: Option<&str>,
    max_output_tokens: u32,
) -> usize {
    let context = model_context_window_tokens(provider_id, model);
    let output_reserve = ((max_output_tokens as f64) / (1.0 - OUTPUT_MARGIN_RATIO)).ceil() as usize;
    let output_reserve = output_reserve.max((context as f64 * OUTPUT_MARGIN_RATIO) as usize);
    context
        .saturating_sub(output_reserve)
        .saturating_sub(PROMPT_OVERHEAD_TOKENS)
}

/// Indique si le texte dépasse la fenêtre effective et nécessite le pipeline map-reduce.
pub fn needs_map_reduce_pipeline(
    text: &str,
    provider_id: &str,
    model: Option<&str>,
    max_output_tokens: u32,
) -> bool {
    let budget = effective_input_token_budget(provider_id, model, max_output_tokens);
    estimate_tokens(text) > budget
}

/// Convertit un budget tokens en limite de caractères pour le découpage.
pub fn token_budget_to_char_limit(token_budget: usize) -> usize {
    token_budget.saturating_mul(CHARS_PER_TOKEN)
}

/// Estime le coût cloud approximatif (USD) pour un nombre de tokens entrée/sortie.
pub fn estimate_cost_usd(
    provider_id: &str,
    model: Option<&str>,
    input_tokens: u64,
    output_tokens: u64,
) -> f64 {
    let model = model.unwrap_or("");
    let (input_rate, output_rate) = match provider_id {
        "mistral" if model.contains("medium") => {
            (MISTRAL_MEDIUM_INPUT_USD_PER_M, MISTRAL_MEDIUM_OUTPUT_USD_PER_M)
        }
        "mistral" => (MISTRAL_SMALL_INPUT_USD_PER_M, MISTRAL_SMALL_OUTPUT_USD_PER_M),
        "openai" if model.contains("gpt-4o-mini") || model.contains("4.1-mini") => {
            (OPENAI_GPT4O_MINI_INPUT_USD_PER_M, OPENAI_GPT4O_MINI_OUTPUT_USD_PER_M)
        }
        "openai" => (OPENAI_GPT4O_INPUT_USD_PER_M, OPENAI_GPT4O_OUTPUT_USD_PER_M),
        "ollama" | _ => (OLLAMA_LOCAL_USD_PER_M, OLLAMA_LOCAL_USD_PER_M),
    };

    let input_cost = input_tokens as f64 * input_rate / 1_000_000.0;
    let output_cost = output_tokens as f64 * output_rate / 1_000_000.0;
    ((input_cost + output_cost) * 100.0).round() / 100.0
}

/// Métadonnées optionnelles exposées au frontend après génération.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryPipelineMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pipeline_used: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_input_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estimated_cost_usd: Option<f64>,
}

/// Phase du pipeline pour logs / progression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelinePhase {
    Chunking,
    Map { index: usize, total: usize },
    Reduce,
}

impl std::fmt::Display for PipelinePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chunking => write!(f, "chunking"),
            Self::Map { index, total } => write!(f, "map {index}/{total}"),
            Self::Reduce => write!(f, "merge"),
        }
    }
}

/// Découpe une transcription aux frontières de segments locuteur / paragraphes.
/// Retourne une erreur si un segment isolé dépasse `max_chars` (pas de troncature silencieuse).
pub fn split_transcription(
    text: &str,
    max_chars: usize,
    overlap_segments: usize,
) -> Result<Vec<String>, AiError> {
    if max_chars == 0 {
        return Err(AiError::Other(
            "budget de découpage invalide (0 caractères)".into(),
        ));
    }

    let segments = parse_segments(text);
    if segments.is_empty() {
        return Err(AiError::Other("transcription vide après découpage".into()));
    }

    for segment in &segments {
        if segment.chars().count() > max_chars {
            return split_oversized_segment(segment, max_chars, overlap_segments);
        }
    }

    pack_segments(&segments, max_chars, overlap_segments)
}

fn parse_segments(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return vec![];
    }

    let mut segments = Vec::new();
    let mut current = String::new();

    for line in trimmed.lines() {
        let is_speaker_line = is_speaker_boundary(line);
        if is_speaker_line && !current.is_empty() {
            segments.push(current.trim_end().to_string());
            current.clear();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
    }
    if !current.trim().is_empty() {
        segments.push(current.trim_end().to_string());
    }

    if segments.len() <= 1 && !trimmed.contains('\n') {
        return vec![trimmed.to_string()];
    }

    if segments.is_empty() {
        for paragraph in trimmed.split("\n\n") {
            let p = paragraph.trim();
            if !p.is_empty() {
                segments.push(p.to_string());
            }
        }
    }

    if segments.is_empty() {
        segments.push(trimmed.to_string());
    }

    segments
}

fn is_speaker_boundary(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('[') && trimmed.contains(']')
}

fn split_oversized_segment(
    segment: &str,
    max_chars: usize,
    overlap_segments: usize,
) -> Result<Vec<String>, AiError> {
    let prefix = speaker_prefix(segment);
    let body = segment.strip_prefix(&prefix).unwrap_or(segment).trim_start();

    let prefix_chars = prefix.chars().count();
    let body_budget = max_chars.saturating_sub(prefix_chars).max(1);

    let mut parts = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = body.chars().collect();

    while start < chars.len() {
        let end = (start + body_budget).min(chars.len());
        let slice: String = chars[start..end].iter().collect();
        let part = if prefix.is_empty() {
            slice
        } else {
            format!("{prefix}{slice}")
        };
        parts.push(part);
        if end >= chars.len() {
            break;
        }
        start = end;
    }

    if parts.is_empty() {
        return Err(AiError::Other(
            "impossible de découper un segment trop long sans troncature".into(),
        ));
    }

    pack_segments(&parts, max_chars, overlap_segments)
}

fn speaker_prefix(segment: &str) -> String {
    let trimmed = segment.trim_start();
    if let Some(end) = trimmed.find(']') {
        format!("{}] ", &trimmed[..=end])
    } else {
        String::new()
    }
}

fn pack_segments(
    segments: &[String],
    max_chars: usize,
    overlap_segments: usize,
) -> Result<Vec<String>, AiError> {
    if segments.is_empty() {
        return Err(AiError::Other("aucun segment à regrouper".into()));
    }

    let join_indices = |indices: &[usize]| -> String {
        indices
            .iter()
            .map(|&i| segments[i].as_str())
            .collect::<Vec<_>>()
            .join("\n")
    };

    let char_len = |indices: &[usize]| -> usize {
        let mut len = 0usize;
        for (pos, &idx) in indices.iter().enumerate() {
            if pos > 0 {
                len += 1;
            }
            len += segments[idx].chars().count();
        }
        len
    };

    let mut chunks: Vec<String> = Vec::new();
    let mut current: Vec<usize> = Vec::new();

    for idx in 0..segments.len() {
        let mut trial = current.clone();
        trial.push(idx);
        if char_len(&trial) > max_chars && !current.is_empty() {
            chunks.push(join_indices(&current));
            let overlap_start = current.len().saturating_sub(overlap_segments);
            current = current[overlap_start..].to_vec();
            if current.contains(&idx) {
                current.retain(|&i| i != idx);
            }
            current.push(idx);
        } else {
            current.push(idx);
        }
    }

    if !current.is_empty() {
        chunks.push(join_indices(&current));
    }

    if chunks.is_empty() {
        return Err(AiError::Other(
            "découpage impossible : vérifiez la taille des segments".into(),
        ));
    }

    Ok(chunks)
}

fn normalize_key(value: &str) -> String {
    value.trim().to_lowercase()
}

fn dedup_strings_ordered(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for value in values {
        let key = normalize_key(&value);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        out.push(value);
    }
    out
}

fn dedup_actions_ordered(actions: Vec<StructuredActionItem>) -> Vec<StructuredActionItem> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for action in actions {
        let key = normalize_key(&action.titre);
        if key.is_empty() || seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        out.push(action);
    }
    out
}

/// Fusionne des comptes-rendus partiels en conservant l'ordre chronologique et en dédupliquant.
pub fn merge_partial_summaries(partials: &[StructuredSummary]) -> StructuredSummary {
    if partials.is_empty() {
        return StructuredSummary {
            synthese: String::new(),
            decisions: vec![],
            actions: vec![],
            risques: vec![],
            questions_ouvertes: vec![],
        };
    }

    if partials.len() == 1 {
        return partials[0].clone();
    }

    let synthese = partials
        .iter()
        .map(|p| p.synthese.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    let decisions = dedup_strings_ordered(
        partials
            .iter()
            .flat_map(|p| p.decisions.clone())
            .collect(),
    );
    let actions = dedup_actions_ordered(
        partials
            .iter()
            .flat_map(|p| p.actions.clone())
            .collect(),
    );
    let risques = dedup_strings_ordered(
        partials
            .iter()
            .flat_map(|p| p.risques.clone())
            .collect(),
    );
    let questions_ouvertes = dedup_strings_ordered(
        partials
            .iter()
            .flat_map(|p| p.questions_ouvertes.clone())
            .collect(),
    );

    StructuredSummary {
        synthese,
        decisions,
        actions,
        risques,
        questions_ouvertes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_uses_chars_div_four() {
        assert_eq!(estimate_tokens(""), 1);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
        assert_eq!(estimate_tokens("éàü"), 1);
    }

    #[test]
    fn model_context_window_returns_known_values() {
        assert_eq!(
            model_context_window_tokens("mistral", Some("mistral-small-latest")),
            32_768
        );
        assert_eq!(
            model_context_window_tokens("openai", Some("gpt-4o-mini")),
            128_000
        );
        assert_eq!(model_context_window_tokens("ollama", None), 8_192);
    }

    #[test]
    fn needs_pipeline_when_text_exceeds_budget() {
        let tiny_budget_provider = "ollama";
        let long_text = "x".repeat(40_000);
        assert!(needs_map_reduce_pipeline(
            &long_text,
            tiny_budget_provider,
            None,
            4096
        ));
        assert!(!needs_map_reduce_pipeline("court", tiny_budget_provider, None, 4096));
    }

    #[test]
    fn split_respects_speaker_boundaries() {
        let text = "[SPEAKER_00 0.0s–1.0s] Bonjour.\n[SPEAKER_01 1.0s–2.0s] Salut.\n[SPEAKER_00 2.0s–3.0s] OK.";
        let chunks = split_transcription(text, 80, 1).expect("split");
        assert!(chunks.len() >= 1);
        for chunk in &chunks {
            assert!(chunk.contains('['));
        }
    }

    #[test]
    fn split_utf8_multibyte_chars() {
        let text = "[SPEAKER_00] ".to_string() + &"é".repeat(200);
        let chunks = split_transcription(&text, 100, 0).expect("split utf8");
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            assert!(chunk.is_char_boundary(chunk.len()));
        }
    }

    #[test]
    fn split_very_long_speaker_turn_without_silent_truncate() {
        let body = "mot ".repeat(500);
        let text = format!("[SPEAKER_00 0.0s–100.0s] {body}");
        let chunks = split_transcription(&text, 200, 0).expect("split long turn");
        assert!(chunks.len() > 1);
        let rejoined_len: usize = chunks.iter().map(|c| c.chars().count()).sum();
        assert!(rejoined_len >= text.chars().count());
    }

    #[test]
    fn split_overlap_includes_trailing_segments() {
        let lines: Vec<String> = (0..6)
            .map(|i| format!("[SPEAKER_{i:02} {i}.0s–{i}.5s] segment {i}"))
            .collect();
        let text = lines.join("\n");
        let chunks = split_transcription(&text, 60, 2).expect("split overlap");
        assert!(chunks.len() >= 2);
    }

    #[test]
    fn merge_dedups_case_insensitive_preserving_order() {
        let a = StructuredSummary {
            synthese: "Partie A.".into(),
            decisions: vec!["Valider Q4".into(), "Reporter budget".into()],
            actions: vec![StructuredActionItem {
                titre: "Envoyer devis".into(),
                description: None,
                responsable: None,
                echeance: None,
            }],
            risques: vec![],
            questions_ouvertes: vec![],
        };
        let b = StructuredSummary {
            synthese: "Partie B.".into(),
            decisions: vec!["valider q4".into(), "Lancer pilote".into()],
            actions: vec![StructuredActionItem {
                titre: "  envoyer devis  ".into(),
                description: None,
                responsable: Some("Marie".into()),
                echeance: None,
            }],
            risques: vec!["Retard".into()],
            questions_ouvertes: vec![],
        };
        let merged = merge_partial_summaries(&[a, b]);
        assert_eq!(merged.decisions.len(), 3);
        assert_eq!(merged.decisions[0], "Valider Q4");
        assert_eq!(merged.actions.len(), 1);
        assert!(merged.synthese.contains("Partie A."));
        assert!(merged.synthese.contains("Partie B."));
    }

    #[test]
    fn estimate_cost_is_documented_and_non_negative() {
        let cost = estimate_cost_usd("mistral", Some("mistral-small-latest"), 100_000, 4_096);
        assert!(cost >= 0.0);
        let local = estimate_cost_usd("ollama", None, 1_000_000, 1_000_000);
        assert_eq!(local, 0.0);
    }

    #[test]
    fn rejects_zero_char_budget() {
        assert!(split_transcription("hello", 0, 0).is_err());
    }
}
