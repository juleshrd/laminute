//! Cartographie locuteurs (labels techniques → noms confirmés).

use std::collections::HashMap;

use crate::ai::structured_summary::StructuredSummary;

pub type SpeakerMap = HashMap<String, String>;

pub fn format_speaker_identity_block(pairs: &[(String, String)]) -> String {
    if pairs.is_empty() {
        return String::new();
    }

    let mut lines = vec![
        "Identité des locuteurs (label technique → nom confirmé, pour interpréter la transcription) :"
            .to_string(),
    ];
    for (id, name) in pairs {
        if name.trim().is_empty() {
            continue;
        }
        lines.push(format!("- {id} → {name}"));
    }
    if lines.len() == 1 {
        return String::new();
    }
    lines.join("\n") + "\n\n"
}

pub fn substitute_label(label: &str, map: &SpeakerMap) -> String {
    map.get(label)
        .filter(|name| !name.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| label.to_string())
}

pub fn substitute_in_text(text: &str, map: &SpeakerMap) -> String {
    if map.is_empty() {
        return text.to_string();
    }

    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort_by_key(|b| std::cmp::Reverse(b.len()));

    let mut result = text.to_string();
    for key in keys {
        let name = &map[key];
        if name.trim().is_empty() {
            continue;
        }
        result = result.replace(key, name);
    }
    result
}

pub fn apply_speaker_map_to_structured(summary: &mut StructuredSummary, map: &SpeakerMap) {
    if map.is_empty() {
        return;
    }

    summary.synthese = substitute_in_text(&summary.synthese, map);
    for decision in &mut summary.decisions {
        *decision = substitute_in_text(decision, map);
    }
    for action in &mut summary.actions {
        if let Some(ref responsable) = action.responsable {
            action.responsable = Some(substitute_label(responsable, map));
        }
        if let Some(ref description) = action.description {
            action.description = Some(substitute_in_text(description, map));
        }
    }
    for risk in &mut summary.risques {
        *risk = substitute_in_text(risk, map);
    }
    for question in &mut summary.questions_ouvertes {
        *question = substitute_in_text(question, map);
    }
}
pub fn speaker_identity_pairs(map: &SpeakerMap) -> Vec<(String, String)> {
    map.iter()
        .filter(|(_, name)| !name.trim().is_empty())
        .map(|(id, name)| (id.clone(), name.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_summary::StructuredActionItem;

    #[test]
    fn format_speaker_identity_block_lists_pairs() {
        let block = format_speaker_identity_block(&[
            ("SPEAKER_00".into(), "Marie".into()),
            ("SPEAKER_01".into(), "Paul".into()),
        ]);
        assert!(block.contains("SPEAKER_00 → Marie"));
        assert!(block.contains("SPEAKER_01 → Paul"));
    }

    #[test]
    fn substitute_label_uses_map() {
        let map = HashMap::from([("SPEAKER_00".into(), "Marie".into())]);
        assert_eq!(substitute_label("SPEAKER_00", &map), "Marie");
        assert_eq!(substitute_label("Alice", &map), "Alice");
    }

    #[test]
    fn apply_speaker_map_substitutes_action_responsable() {
        let map = HashMap::from([("SPEAKER_00".into(), "Marie".into())]);
        let mut summary = StructuredSummary {
            synthese: "Synthèse.".into(),
            decisions: vec![],
            actions: vec![StructuredActionItem {
                titre: "Envoyer le devis".into(),
                description: None,
                responsable: Some("SPEAKER_00".into()),
                echeance: None,
            }],
            risques: vec![],
            questions_ouvertes: vec![],
        };
        apply_speaker_map_to_structured(&mut summary, &map);
        assert_eq!(summary.actions[0].responsable.as_deref(), Some("Marie"));
    }
}
