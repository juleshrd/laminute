# Sécurité

Merci de nous aider à garder **La Minute** sûre.

## Signaler une vulnérabilité

N’ouvrez **pas** d’issue publique pour une faille de sécurité.

1. Préférez [GitHub Security Advisories](https://github.com/juleshrd/laminute/security/advisories/new) sur ce dépôt.
2. Décrivez l’impact, les étapes de reproduction et, si possible, une proposition de correctif.

Nous accuserons réception dès que possible et travaillerons à un correctif avant toute divulgation.

## Périmètre

La Minute est une application desktop locale (Tauri). Les clés API BYOK sont stockées dans le trousseau du système. Les réunions et fichiers audio restent sur la machine de l’utilisateur, sauf envoi volontaire vers le fournisseur IA choisi (transcription / compte-rendu).

## Mises à jour

Les artefacts d’auto-update sont signés Minisign. Vérifiez toujours que vous téléchargez depuis les [releases officielles](https://github.com/juleshrd/laminute/releases).
