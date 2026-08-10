# Releases sans certificats Apple et état Windows

## Objectif

Permettre temporairement la publication de La Minute sans adhésion Apple tout en rendant l’absence de signature explicite, puis documenter le comportement Windows et ses avertissements SmartScreen.

## Plan

- [x] Réintroduire un fallback macOS non signé uniquement lorsque tous les secrets Apple sont absents.
- [x] Rendre le statut signé/non signé visible dans les logs et cohérent dans les notes de release et la documentation.
- [x] Auditer le build Windows actuel, la signature Authenticode et les conséquences SmartScreen.
- [x] Adapter les tests de configuration release et exécuter la validation complète.

## Revue finale

- Aucun secret Apple : le workflow publie le DMG en mode `unsigned`, sans injecter de variable `APPLE_*` vide, et affiche un avertissement explicite dans le résumé GitHub Actions.
- Cinq secrets Apple : le chemin Developer ID/notarisation et ses contrôles restent actifs. Une configuration partielle échoue afin d’éviter une signature ambiguë.
- Windows : l’EXE NSIS reste publiable sans Authenticode. Le workflow contrôle son statut et signale SmartScreen sans bloquer la release.
- Les politiques sont alignées dans README, SECURITY, CONTRIBUTING et les guides macOS/Windows.
- `npm run check` passe : format, lint, TypeScript, Stylelint, Clippy, 113 tests web, 168 tests Rust (3 trousseau ignorés), builds web et Rust.
