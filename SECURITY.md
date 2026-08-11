# Sécurité

Merci de nous aider à garder **La Minute** sûre.

## Signaler une vulnérabilité

N’ouvrez **pas** d’issue publique pour une faille de sécurité.

1. Préférez [GitHub Security Advisories](https://github.com/juleshrd/laminute/security/advisories/new) sur ce dépôt.
2. Décrivez l’impact, les étapes de reproduction et, si possible, une proposition de correctif.

Nous accuserons réception dès que possible et travaillerons à un correctif avant toute divulgation.

## Périmètre

La Minute est une application desktop locale (Tauri). Les clés API BYOK sont stockées dans le trousseau du système. Les réunions et fichiers audio restent sur la machine de l’utilisateur, sauf envoi volontaire vers le fournisseur IA choisi (transcription / compte-rendu).

## Journaux et support

Les journaux locaux sous `logs/` et le bundle de support volontaire (Réglages → Diagnostic) sont conçus pour le dépannage **sans** y inclure de secrets ni de contenu de réunion. Les messages sont expurgés (clés API, jetons Bearer, chemins utilisateur sensibles, corps de transcription). Signalez toute fuite observée dans ces artefacts comme une vulnérabilité de confidentialité.

## Mises à jour

Les artefacts d’auto-update sont signés Minisign. Vérifiez toujours que vous téléchargez depuis les [releases officielles](https://github.com/juleshrd/laminute/releases).

La CI publie les bundles macOS même sans identifiants Apple. Lorsqu’ils sont configurés, elle contrôle la signature Developer ID, l’évaluation Gatekeeper et le ticket de notarisation. Sans eux, le DMG est explicitement publié non signé et non notarié. L’installateur Windows est également publié sans Authenticode pour le moment ; SmartScreen peut donc afficher un avertissement.
