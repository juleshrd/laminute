//! Banc d'évaluation offline transcription + compte-rendu (JUL-199).

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::ai::structured_summary::{parse_structured_summary, StructuredActionItem, StructuredSummary};

/// Scénario d'évaluation chargé depuis le corpus JSON.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalScenario {
    pub id: String,
    pub tags: Vec<String>,
    pub transcription: String,
    pub gold: StructuredSummary,
    pub hypothesis_summary: StructuredSummary,
    #[serde(default)]
    pub transcription_hypothesis: Option<String>,
}

/// Seuils bloquants pour l'évaluation.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalThresholds {
    pub schema_ok_rate: f64,
    pub critical_hallucination_rate: f64,
    pub min_decision_recall: f64,
    pub min_action_recall: f64,
    #[serde(default = "default_min_decision_precision")]
    pub min_decision_precision: f64,
    #[serde(default = "default_min_action_precision")]
    pub min_action_precision: f64,
}

fn default_min_decision_precision() -> f64 {
    0.5
}

fn default_min_action_precision() -> f64 {
    0.5
}

/// Métriques calculées pour un scénario.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioMetrics {
    pub id: String,
    pub tags: Vec<String>,
    pub schema_ok: bool,
    pub decision_precision: f64,
    pub decision_recall: f64,
    pub action_precision: f64,
    pub action_recall: f64,
    pub responsible_accuracy: Option<f64>,
    pub echeance_accuracy: Option<f64>,
    pub critical_hallucination: bool,
    pub wer: Option<f64>,
    pub cer: Option<f64>,
}

/// Agrégats sur l'ensemble du corpus.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregateMetrics {
    pub scenario_count: usize,
    pub schema_ok_rate: f64,
    pub critical_hallucination_rate: f64,
    pub decision_precision: f64,
    pub decision_recall: f64,
    pub action_precision: f64,
    pub action_recall: f64,
    pub responsible_accuracy: Option<f64>,
    pub echeance_accuracy: Option<f64>,
    pub wer_mean: Option<f64>,
    pub cer_mean: Option<f64>,
    pub thresholds_met: bool,
    pub failed_thresholds: Vec<String>,
}

/// Rapport complet d'évaluation.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalReport {
    pub mode: String,
    pub corpus_dir: String,
    pub generated_at: String,
    pub scenarios: Vec<ScenarioMetrics>,
    pub aggregate: AggregateMetrics,
}

/// Charge tous les scénarios depuis `corpus/scenarios/` et `corpus/adversarial/`.
pub fn load_scenarios(corpus_dir: &Path) -> Result<Vec<EvalScenario>, String> {
    let mut scenarios = Vec::new();
    for sub in ["scenarios", "adversarial"] {
        let dir = corpus_dir.join("corpus").join(sub);
        if !dir.is_dir() {
            return Err(format!("répertoire corpus manquant : {}", dir.display()));
        }
        let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
            .map_err(|e| format!("lecture {} : {e}", dir.display()))?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "json"))
            .collect();
        paths.sort();
        for path in paths {
            let raw = fs::read_to_string(&path)
                .map_err(|e| format!("lecture {} : {e}", path.display()))?;
            let scenario: EvalScenario = serde_json::from_str(&raw)
                .map_err(|e| format!("JSON invalide {} : {e}", path.display()))?;
            scenarios.push(scenario);
        }
    }
    if scenarios.is_empty() {
        return Err("aucun scénario trouvé dans le corpus".into());
    }
    Ok(scenarios)
}

pub fn load_thresholds(corpus_dir: &Path) -> Result<EvalThresholds, String> {
    let path = corpus_dir.join("thresholds.json");
    let raw =
        fs::read_to_string(&path).map_err(|e| format!("lecture {} : {e}", path.display()))?;
    serde_json::from_str(&raw).map_err(|e| format!("thresholds.json invalide : {e}"))
}

/// Évalue un scénario offline à partir de son `hypothesisSummary` embarqué.
pub fn evaluate_scenario(scenario: &EvalScenario) -> ScenarioMetrics {
    let hypothesis_json = serde_json::to_string(&scenario.hypothesis_summary)
        .unwrap_or_else(|_| "{}".into());
    let schema_ok = parse_structured_summary(&hypothesis_json).is_ok();

    let gold_decisions = &scenario.gold.decisions;
    let hyp_decisions = &scenario.hypothesis_summary.decisions;
    let gold_decision_refs: Vec<&str> = gold_decisions.iter().map(|d| d.text()).collect();
    let hyp_decision_refs: Vec<&str> = hyp_decisions.iter().map(|d| d.text()).collect();
    let (decision_precision, decision_recall) =
        precision_recall_strings(&gold_decision_refs, &hyp_decision_refs);

    let gold_actions: Vec<&str> = scenario
        .gold
        .actions
        .iter()
        .map(|a| a.titre.as_str())
        .collect();
    let hyp_actions: Vec<&str> = scenario
        .hypothesis_summary
        .actions
        .iter()
        .map(|a| a.titre.as_str())
        .collect();
    let (action_precision, action_recall) = precision_recall_strings(&gold_actions, &hyp_actions);

    let responsible_accuracy = field_accuracy(
        &scenario.gold.actions,
        &scenario.hypothesis_summary.actions,
        |a| a.responsable.as_deref(),
    );
    let echeance_accuracy = field_accuracy(
        &scenario.gold.actions,
        &scenario.hypothesis_summary.actions,
        |a| a.echeance.as_deref(),
    );

    let critical_hallucination = detect_critical_hallucination(scenario);

    let (wer, cer) = match &scenario.transcription_hypothesis {
        Some(hyp) => (
            Some(word_error_rate(&scenario.transcription, hyp)),
            Some(char_error_rate(&scenario.transcription, hyp)),
        ),
        None => (None, None),
    };

    ScenarioMetrics {
        id: scenario.id.clone(),
        tags: scenario.tags.clone(),
        schema_ok,
        decision_precision,
        decision_recall,
        action_precision,
        action_recall,
        responsible_accuracy,
        echeance_accuracy,
        critical_hallucination,
        wer,
        cer,
    }
}

pub fn aggregate_metrics(
    scenarios: &[ScenarioMetrics],
    thresholds: &EvalThresholds,
) -> AggregateMetrics {
    let n = scenarios.len() as f64;
    let schema_ok_rate = scenarios.iter().filter(|s| s.schema_ok).count() as f64 / n;
    let critical_hallucination_rate =
        scenarios.iter().filter(|s| s.critical_hallucination).count() as f64 / n;

    let decision_precision = mean(scenarios.iter().map(|s| s.decision_precision));
    let decision_recall = mean(scenarios.iter().map(|s| s.decision_recall));
    let action_precision = mean(scenarios.iter().map(|s| s.action_precision));
    let action_recall = mean(scenarios.iter().map(|s| s.action_recall));

    let responsible_accuracy = mean_optional(scenarios.iter().filter_map(|s| s.responsible_accuracy));
    let echeance_accuracy = mean_optional(scenarios.iter().filter_map(|s| s.echeance_accuracy));

    let wer_values: Vec<f64> = scenarios.iter().filter_map(|s| s.wer).collect();
    let cer_values: Vec<f64> = scenarios.iter().filter_map(|s| s.cer).collect();
    let wer_mean = if wer_values.is_empty() {
        None
    } else {
        Some(mean(wer_values.iter().copied()))
    };
    let cer_mean = if cer_values.is_empty() {
        None
    } else {
        Some(mean(cer_values.iter().copied()))
    };

    let mut failed_thresholds = Vec::new();
    if schema_ok_rate < thresholds.schema_ok_rate {
        failed_thresholds.push(format!(
            "schema_ok_rate {schema_ok_rate:.3} < {}",
            thresholds.schema_ok_rate
        ));
    }
    if critical_hallucination_rate > thresholds.critical_hallucination_rate {
        failed_thresholds.push(format!(
            "critical_hallucination_rate {critical_hallucination_rate:.3} > {}",
            thresholds.critical_hallucination_rate
        ));
    }
    if decision_recall < thresholds.min_decision_recall {
        failed_thresholds.push(format!(
            "decision_recall {decision_recall:.3} < {}",
            thresholds.min_decision_recall
        ));
    }
    if action_recall < thresholds.min_action_recall {
        failed_thresholds.push(format!(
            "action_recall {action_recall:.3} < {}",
            thresholds.min_action_recall
        ));
    }
    if decision_precision < thresholds.min_decision_precision {
        failed_thresholds.push(format!(
            "decision_precision {decision_precision:.3} < {}",
            thresholds.min_decision_precision
        ));
    }
    if action_precision < thresholds.min_action_precision {
        failed_thresholds.push(format!(
            "action_precision {action_precision:.3} < {}",
            thresholds.min_action_precision
        ));
    }

    AggregateMetrics {
        scenario_count: scenarios.len(),
        schema_ok_rate,
        critical_hallucination_rate,
        decision_precision,
        decision_recall,
        action_precision,
        action_recall,
        responsible_accuracy,
        echeance_accuracy,
        wer_mean,
        cer_mean,
        thresholds_met: failed_thresholds.is_empty(),
        failed_thresholds,
    }
}

pub fn run_offline_eval(corpus_dir: &Path) -> Result<EvalReport, String> {
    let scenarios = load_scenarios(corpus_dir)?;
    let thresholds = load_thresholds(corpus_dir)?;
    let metrics: Vec<ScenarioMetrics> = scenarios.iter().map(evaluate_scenario).collect();
    let aggregate = aggregate_metrics(&metrics, &thresholds);
    Ok(EvalReport {
        mode: "offline".into(),
        corpus_dir: corpus_dir.display().to_string(),
        generated_at: chrono::Utc::now().to_rfc3339(),
        scenarios: metrics,
        aggregate,
    })
}

pub fn report_to_markdown(report: &EvalReport) -> String {
    let agg = &report.aggregate;
    let mut md = String::new();
    md.push_str("# Rapport d'évaluation IA\n\n");
    md.push_str(&format!("- **Mode** : {}\n", report.mode));
    md.push_str(&format!("- **Corpus** : {}\n", report.corpus_dir));
    md.push_str(&format!("- **Généré** : {}\n", report.generated_at));
    md.push_str(&format!(
        "- **Résultat seuils** : {}\n\n",
        if agg.thresholds_met {
            "PASS"
        } else {
            "FAIL"
        }
    ));

    md.push_str("## Agrégats\n\n");
    md.push_str("| Métrique | Valeur |\n");
    md.push_str("| -------- | ------ |\n");
    md.push_str(&format!("| Scénarios | {} |\n", agg.scenario_count));
    md.push_str(&format!("| schema_ok_rate | {:.3} |\n", agg.schema_ok_rate));
    md.push_str(&format!(
        "| critical_hallucination_rate | {:.3} |\n",
        agg.critical_hallucination_rate
    ));
    md.push_str(&format!("| decision_recall | {:.3} |\n", agg.decision_recall));
    md.push_str(&format!("| decision_precision | {:.3} |\n", agg.decision_precision));
    md.push_str(&format!("| action_recall | {:.3} |\n", agg.action_recall));
    md.push_str(&format!("| action_precision | {:.3} |\n", agg.action_precision));
    if let Some(v) = agg.responsible_accuracy {
        md.push_str(&format!("| responsible_accuracy | {:.3} |\n", v));
    }
    if let Some(v) = agg.echeance_accuracy {
        md.push_str(&format!("| echeance_accuracy | {:.3} |\n", v));
    }
    if let Some(v) = agg.wer_mean {
        md.push_str(&format!("| wer_mean | {:.3} |\n", v));
    }
    if let Some(v) = agg.cer_mean {
        md.push_str(&format!("| cer_mean | {:.3} |\n", v));
    }

    if !agg.failed_thresholds.is_empty() {
        md.push_str("\n## Seuils non atteints\n\n");
        for t in &agg.failed_thresholds {
            md.push_str(&format!("- {t}\n"));
        }
    }

    md.push_str("\n## Détail par scénario\n\n");
    md.push_str("| id | schema | dec R/P | act R/P | hallucination |\n");
    md.push_str("| -- | ------ | ------- | ------- | ------------- |\n");
    for s in &report.scenarios {
        md.push_str(&format!(
            "| {} | {} | {:.2}/{:.2} | {:.2}/{:.2} | {} |\n",
            s.id,
            if s.schema_ok { "ok" } else { "KO" },
            s.decision_recall,
            s.decision_precision,
            s.action_recall,
            s.action_precision,
            if s.critical_hallucination {
                "oui"
            } else {
                "non"
            }
        ));
    }
    md
}

/// Normalise un texte pour le matching (lowercase, trim, accents optionnels).
pub fn normalize_text(text: &str) -> String {
    let lowered = text.trim().to_lowercase();
    strip_accents(&lowered)
}

fn strip_accents(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ä' | 'ã' => 'a',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            'œ' => 'o',
            'æ' => 'a',
            _ => c,
        })
        .collect()
}

fn strings_match(a: &str, b: &str) -> bool {
    let na = normalize_text(a);
    let nb = normalize_text(b);
    if na.is_empty() || nb.is_empty() {
        return false;
    }
    na == nb || na.contains(&nb) || nb.contains(&na)
}

fn precision_recall_strings(gold: &[&str], hypothesis: &[&str]) -> (f64, f64) {
    if hypothesis.is_empty() && gold.is_empty() {
        return (1.0, 1.0);
    }

    let matched_hyp = hypothesis
        .iter()
        .filter(|h| gold.iter().any(|g| strings_match(h, g)))
        .count();
    let matched_gold = gold
        .iter()
        .filter(|g| hypothesis.iter().any(|h| strings_match(h, g)))
        .count();

    let precision = if hypothesis.is_empty() {
        1.0
    } else {
        matched_hyp as f64 / hypothesis.len() as f64
    };
    let recall = if gold.is_empty() {
        1.0
    } else {
        matched_gold as f64 / gold.len() as f64
    };
    (precision, recall)
}

fn field_accuracy<F>(
    gold_actions: &[StructuredActionItem],
    hyp_actions: &[StructuredActionItem],
    field: F,
) -> Option<f64>
where
    F: Fn(&StructuredActionItem) -> Option<&str>,
{
    let mut total = 0usize;
    let mut correct = 0usize;
    for g in gold_actions {
        let expected = field(g)?;
        if expected.trim().is_empty() {
            continue;
        }
        total += 1;
        if let Some(h) = hyp_actions
            .iter()
            .find(|a| strings_match(&a.titre, &g.titre))
        {
            if let Some(actual) = field(h) {
                if strings_match(expected, actual) {
                    correct += 1;
                }
            }
        }
    }
    if total == 0 {
        None
    } else {
        Some(correct as f64 / total as f64)
    }
}

/// Détecte une hallucination critique : item hypothèse absent du gold et non ancré dans la transcription.
pub fn detect_critical_hallucination(scenario: &EvalScenario) -> bool {
    let transcription_norm = normalize_text(&scenario.transcription);

    for decision in &scenario.hypothesis_summary.decisions {
        if scenario
            .gold
            .decisions
            .iter()
            .any(|g| strings_match(g.text(), decision.text()))
        {
            continue;
        }
        if !anchored_in_transcription(decision.text(), &transcription_norm) {
            return true;
        }
    }

    for action in &scenario.hypothesis_summary.actions {
        if scenario
            .gold
            .actions
            .iter()
            .any(|g| strings_match(&g.titre, &action.titre))
        {
            continue;
        }
        if !anchored_in_transcription(&action.titre, &transcription_norm) {
            return true;
        }
    }

    false
}

fn anchored_in_transcription(text: &str, transcription_norm: &str) -> bool {
    let normalized = normalize_text(text);
    let tokens: Vec<&str> = normalized
        .split_whitespace()
        .filter(|t| t.len() > 3)
        .collect();
    if tokens.is_empty() {
        return true;
    }
    tokens
        .iter()
        .any(|t| transcription_norm.contains(t))
}

fn mean<I>(values: I) -> f64
where
    I: Iterator<Item = f64>,
{
    let mut count = 0usize;
    let mut sum = 0.0;
    for v in values {
        sum += v;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f64
    }
}

fn mean_optional<I>(values: I) -> Option<f64>
where
    I: Iterator<Item = f64>,
{
    let mut count = 0usize;
    let mut sum = 0.0;
    for v in values {
        sum += v;
        count += 1;
    }
    if count == 0 {
        None
    } else {
        Some(sum / count as f64)
    }
}

/// Word Error Rate (Levenshtein au niveau des mots).
pub fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let ref_words: Vec<&str> = reference.split_whitespace().collect();
    let hyp_words: Vec<&str> = hypothesis.split_whitespace().collect();
    if ref_words.is_empty() {
        return if hyp_words.is_empty() { 0.0 } else { 1.0 };
    }
    let dist = levenshtein(&ref_words, &hyp_words);
    dist as f64 / ref_words.len() as f64
}

/// Character Error Rate.
pub fn char_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let ref_chars: Vec<char> = normalize_text(reference).chars().collect();
    let hyp_chars: Vec<char> = normalize_text(hypothesis).chars().collect();
    if ref_chars.is_empty() {
        return if hyp_chars.is_empty() { 0.0 } else { 1.0 };
    }
    let dist = levenshtein(&ref_chars, &hyp_chars);
    dist as f64 / ref_chars.len() as f64
}

fn levenshtein<T: Eq>(a: &[T], b: &[T]) -> usize {
    let m = a.len();
    let n = b.len();
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];
    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        prev.clone_from_slice(&curr);
    }
    prev[n]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::structured_summary::StructuredSummary;

    fn scenario_with(
        transcription: &str,
        gold_decisions: Vec<&str>,
        hyp_decisions: Vec<&str>,
        gold_actions: Vec<(&str, Option<&str>, Option<&str>)>,
        hyp_actions: Vec<(&str, Option<&str>, Option<&str>)>,
    ) -> EvalScenario {
        EvalScenario {
            id: "test".into(),
            tags: vec![],
            transcription: transcription.into(),
            gold: StructuredSummary {
                synthese: "Synthèse gold.".into(),
                decisions: gold_decisions.into_iter().map(|s| s.into()).collect(),
                actions: gold_actions
                    .into_iter()
                    .map(|(titre, resp, ech)| StructuredActionItem {
                        titre: titre.into(),
                        description: None,
                        responsable: resp.map(|s| s.to_string()),
                        echeance: ech.map(|s| s.to_string()),
                ..Default::default()
                    })
                    .collect(),
                risques: vec![],
                questions_ouvertes: vec![],
            },
            hypothesis_summary: StructuredSummary {
                synthese: "Synthèse hypothèse.".into(),
                decisions: hyp_decisions.into_iter().map(|s| s.into()).collect(),
                actions: hyp_actions
                    .into_iter()
                    .map(|(titre, resp, ech)| StructuredActionItem {
                        titre: titre.into(),
                        description: None,
                        responsable: resp.map(|s| s.to_string()),
                        echeance: ech.map(|s| s.to_string()),
                ..Default::default()
                    })
                    .collect(),
                risques: vec![],
                questions_ouvertes: vec![],
            },
            transcription_hypothesis: None,
        }
    }

    #[test]
    fn strings_match_is_case_and_accent_insensitive() {
        assert!(strings_match("Valider le planning", "valider le planning"));
        assert!(strings_match("été", "ete"));
    }

    #[test]
    fn hallucinated_decision_is_detected() {
        let scenario = scenario_with(
            "On discute du budget marketing sans décision.",
            vec![],
            vec!["Lancer la fusion avec Acme Corp"],
            vec![],
            vec![],
        );
        assert!(detect_critical_hallucination(&scenario));
    }

    #[test]
    fn decision_mentioned_in_transcription_is_not_hallucination() {
        let scenario = scenario_with(
            "Nous devons relancer le client Dufour avant vendredi.",
            vec![],
            vec!["Relancer le client Dufour"],
            vec![],
            vec![],
        );
        assert!(!detect_critical_hallucination(&scenario));
    }

    #[test]
    fn responsible_accuracy_counts_matches() {
        let scenario = scenario_with(
            "Marie envoie le devis demain.",
            vec![],
            vec![],
            vec![("Envoyer le devis", Some("Marie"), Some("demain"))],
            vec![("Envoyer le devis", Some("Marie"), Some("demain"))],
        );
        let m = evaluate_scenario(&scenario);
        assert_eq!(m.responsible_accuracy, Some(1.0));
        assert_eq!(m.echeance_accuracy, Some(1.0));
    }

    #[test]
    fn wer_computes_reasonable_value() {
        let wer = word_error_rate("bonjour le monde", "bonjour monde");
        assert!(wer > 0.0 && wer < 1.0);
    }

    #[test]
    fn significant_tokens_appear_in_transcription() {
        let norm = normalize_text("relancer le client Dufour avant vendredi");
        let normalized = normalize_text("Relancer Dufour");
        let tokens: Vec<&str> = normalized
            .split_whitespace()
            .filter(|t| t.len() > 3)
            .filter(|t| norm.contains(*t))
            .collect();
        assert!(!tokens.is_empty());
    }
}
