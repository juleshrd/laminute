# Banc d'évaluation IA — transcription & compte-rendu

Évaluation **reproductible offline** de la qualité des comptes-rendus structurés (et transcription si hypothèse fournie), avant d'optimiser prompts ou modèles.

## Lancer l'évaluation

Depuis la racine du dépôt :

```bash
cd src-tauri
cargo run --example eval_ai -- --mode offline --corpus-dir ../eval --out-dir ../reports/eval
```

La commande écrit `reports/eval/latest.json` et `reports/eval/latest.md`, affiche un résumé Markdown, et retourne un **code de sortie non nul** si les seuils de `eval/thresholds.json` ne sont pas atteints.

### Mettre à jour la baseline versionnée

```bash
cargo run --example eval_ai -- --mode offline --corpus-dir ../eval --out-dir ../reports/eval --write-baseline
```

Cela régénère aussi `reports/eval/baseline.json` et `reports/eval/baseline.md` (à committer lors d'un changement intentionnel du corpus ou des seuils).

## Mode live (opt-in, hors CI)

Le mode live appellerait les providers IA avec les clés locales (BYOK). **Non implémenté dans le MVP** : la commande refuse proprement avec un message explicite. Utiliser uniquement `--mode offline` en CI et pour les PR.

Variable d'environnement reconnue mais refusée pour le MVP : `LAMINUTE_EVAL_LIVE=1`.

## Corpus

| Répertoire | Contenu |
| ---------- | ------- |
| `eval/corpus/scenarios/` | ≥ 20 scénarios métier (texte uniquement, pas d'audio) |
| `eval/corpus/adversarial/` | ≥ 5 scénarios anti-injection (transcription piégée, hypothèse correcte) |
| `eval/fixtures/summary/` | Sorties rejouables offline (miroir des `hypothesisSummary`) |
| `eval/thresholds.json` | Seuils bloquants |

Chaque scénario JSON contient : `transcription`, `gold` (référence), `hypothesisSummary` (sortie modèle simulée offline), et optionnellement `transcriptionHypothesis` (pour WER/CER).

### Tags couverts

`short`, `long`, `fr`, `en`, `bilingual`, `noise`, `multi`, `relative_dates`, `disagreement`, `no_decision`, `adversarial`.

## Métriques

Par scénario :

- **schema_ok** — `hypothesisSummary` valide via `StructuredSummary::validate`
- **decision/action precision & recall** — matching normalisé (casse, accents, containment)
- **responsible_accuracy / echeance_accuracy** — quand le gold renseigne ces champs
- **critical_hallucination** — décision/action hypothèse non présente dans le gold et non ancrée dans la transcription
- **WER / CER** — uniquement si `transcriptionHypothesis` est renseigné, sinon N/A

## Seuils (exemple)

Voir `eval/thresholds.json` : `schema_ok_rate=1.0`, `critical_hallucination_rate=0`, rappels/précisions minimaux sur décisions et actions.

## Convention PR

1. Modifier le corpus ou les seuils → relancer l'eval offline localement.
2. Si les métriques changent intentionnellement, mettre à jour la baseline (`--write-baseline`) et décrire le changement dans la PR.
3. La CI exécute l'eval offline ; un échec de seuil bloque le merge.

## CI

Le job `test_rust` exécute :

```bash
cargo run --manifest-path src-tauri/Cargo.toml --example eval_ai -- --mode offline --corpus-dir eval --out-dir reports/eval
```
