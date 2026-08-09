# Premier lancement macOS — plan d’implémentation

## Objectif

Faire du premier lancement de La Minute un parcours fiable et compréhensible : l’application distribuée sur macOS doit être signée et notariée, les répertoires locaux doivent être prêts avant l’entrée dans l’application, le microphone par défaut doit réellement être sélectionné, et l’utilisateur doit connaître et maîtriser la conservation de ses productions.

## Principes retenus

- Conserver les audios dans les racines privées gérées par l’application (`imports/` et `recordings/`) afin de préserver les protections JUL-174/JUL-181.
- Présenter ces emplacements et la politique de conservation pendant l’onboarding, sans demander à l’utilisateur de comprendre `Application Support`.
- Initialiser les dossiers côté Rust et remonter un diagnostic structuré au frontend ; le WebView ne reçoit aucun droit d’écriture arbitraire.
- Sélectionner et persister automatiquement le microphone système par défaut au premier lancement, avec repli sur le premier périphérique disponible.
- Versionner l’onboarding afin que les installations existantes bénéficient du nouveau parcours une fois, puis permettre de le rejouer depuis les réglages.
- Faire échouer la publication macOS si les secrets Apple nécessaires à la signature/notarisation sont absents ; une release non notariée ne doit plus être annoncée comme stable.

## Checklist

- [x] Ajouter un diagnostic/initialisateur natif de premier lancement : base SQLite, imports, enregistrements, réglages et statut micro.
- [x] Corriger la sélection du microphone par défaut pour que le bouton d’enregistrement fonctionne réellement sur une installation fraîche.
- [x] Remplacer l’onboarding IA isolé par un parcours court : accueil, stockage & confidentialité, IA, confirmation prête à l’emploi.
- [x] Afficher les chemins réellement utilisés, la conservation audio et les états succès/erreur avec possibilité de réessayer.
- [x] Versionner la préférence d’onboarding et préserver l’action « Relancer » dans les réglages.
- [x] Brancher les secrets Apple dans la CI, exiger signature + notarisation sur le job macOS, puis vérifier signature, ticket et Gatekeeper avant publication.
- [x] Mettre à jour la documentation de release sans procédure de contournement Gatekeeper.
- [x] Ajouter les tests Rust et React couvrant installation fraîche, micro par défaut, préparation des dossiers, reprise sur erreur et onboarding versionné.
- [x] Exécuter format, lint, tests, build et un parcours E2E visuel dans la fenêtre Tauri.

## Revue finale

- `npm run check` passe intégralement : format, ESLint, TypeScript, Stylelint, Clippy, 104 tests web, 167 tests Rust (3 tests de trousseau ignorés) et builds web/Rust.
- Le bundle de production macOS a été ouvert et le parcours 5 étapes a été joué jusqu’à l’écran principal : chemins locaux affichés, microphone Mac sélectionné, conservation audio active, Mistral recommandé et mode limité sans clé validés.
- Le build local du bundle produit bien `La Minute.app`; sa dernière étape updater échoue volontairement sans `TAURI_SIGNING_PRIVATE_KEY`, secret déjà exigé par la CI de release.
- La prochaine release macOS ne peut plus être rendue publique tant que signature Developer ID, notarisation, ticket agrafé, évaluation Gatekeeper et vérification DMG ne sont pas tous passés.
