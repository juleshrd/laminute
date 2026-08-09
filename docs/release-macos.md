# Publier La Minute sur macOS

Pour être reconnu par Gatekeeper, un DMG distribué directement exige deux mécanismes Apple distincts :

- une signature **Developer ID Application**, qui lie l’application à l’éditeur ;
- une **notarisation**, qui fait analyser l’artefact par Apple et fournit le ticket vérifié par Gatekeeper.

La signature Minisign de l’auto-update Tauri ne remplace aucun de ces mécanismes.

## Mode temporaire sans adhésion

La CI autorise la publication sans identifiants Apple. Si les cinq secrets `APPLE_*` sont tous absents, elle construit le DMG sans leur transmettre de valeur vide et indique le mode `unsigned` dans le résumé du workflow.

Conséquences attendues :

- le DMG n’est ni signé Developer ID ni notarié ;
- Gatekeeper peut afficher qu’Apple ne peut pas vérifier l’absence de logiciel malveillant ;
- l’utilisateur doit autoriser manuellement l’application dans les réglages de sécurité macOS ;
- Minisign continue de protéger les artefacts d’auto-update, mais n’est pas reconnu par Gatekeeper.

Si seulement une partie des secrets Apple est configurée, la CI échoue volontairement. Cela évite de lancer Tauri avec une identité ou des identifiants de notarisation incomplets.

## Prérequis Apple

1. Utiliser une adhésion payante et active à l’Apple Developer Program. Un compte gratuit ne permet pas de notariser une application distribuée hors App Store.
2. Sur le Mac qui créera le certificat, ouvrir **Trousseaux d’accès > Assistant de certification > Demander un certificat à une autorité de certification**, renseigner l’adresse Apple du détenteur et enregistrer le CSR sur le disque.
3. Dans [Certificates, Identifiers & Profiles](https://developer.apple.com/account/resources/certificates/list), créer un certificat **Developer ID Application** avec ce CSR. Seul l’Account Holder peut créer ce type de certificat.
4. Télécharger puis ouvrir le fichier `.cer`. Dans **Trousseaux d’accès > Mes certificats**, vérifier que le certificat possède bien une clé privée dépliable.
5. Exporter le certificat **avec sa clé privée** en `.p12` et choisir un mot de passe fort et unique.

`Developer ID Installer` concerne les paquets `.pkg` et n’est pas nécessaire pour le DMG de La Minute. Les étapes Apple de référence sont détaillées dans [Developer ID certificates](https://developer.apple.com/help/account/certificates/create-developer-id-certificates/) et la [documentation Tauri macOS](https://v2.tauri.app/distribute/sign/macos/).

## Préparer les secrets GitHub

Encoder le `.p12` sur le Mac :

```bash
openssl base64 -A -in /chemin/Developer-ID-Application.p12 -out certificate-base64.txt
```

Ne jamais ajouter le `.p12`, son mot de passe ou `certificate-base64.txt` au dépôt.

Dans GitHub, ouvrir **Settings > Environments > release > Environment secrets** et créer :

| Secret                               | Valeur                                                                                               |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| `APPLE_CERTIFICATE`                  | Contenu mono-ligne de `certificate-base64.txt`                                                       |
| `APPLE_CERTIFICATE_PASSWORD`         | Mot de passe choisi lors de l’export `.p12`                                                          |
| `APPLE_ID`                           | Adresse du compte Apple utilisé pour la notarisation                                                 |
| `APPLE_PASSWORD`                     | [Mot de passe spécifique à l’app](https://support.apple.com/102654), pas le mot de passe principal   |
| `APPLE_TEAM_ID`                      | Team ID visible dans [Membership details](https://developer.apple.com/account#MembershipDetailsCard) |
| `TAURI_SIGNING_PRIVATE_KEY`          | Clé privée Minisign de l’updater, déjà utilisée par le projet                                        |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | Mot de passe de cette clé updater                                                                    |

`APPLE_SIGNING_IDENTITY` n’est pas nécessaire dans le workflow actuel : Tauri déduit l’identité du certificat importé.

## Publier une version

Le workflow choisit automatiquement le mode `signed-notarized` lorsque les cinq secrets Apple sont présents. S’ils sont tous absents, il utilise le mode `unsigned` décrit plus haut.

1. Aligner la nouvelle version dans `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` et `src-tauri/tauri.conf.json`.
2. Faire valider les changements sur `main`.
3. Créer un **nouveau** tag, par exemple `v0.1.2`, puis le pousser. Ne jamais déplacer ou réutiliser un tag déjà public.
4. Attendre le workflow **Release**. La release reste en brouillon pendant la construction.
5. Vérifier dans le résumé du job `build-macos-latest` si le mode obtenu est `unsigned` ou `signed-notarized`.

En mode signé, le workflow vérifie notamment :

- la chaîne de signature Developer ID ;
- l’entitlement `com.apple.security.device.audio-input` ;
- l’évaluation Gatekeeper de l’application et du DMG ;
- les tickets de notarisation agrafés à l’application et au DMG ;
- l’intégrité du DMG.

## Vérification manuelle d’un DMG téléchargé

Après avoir téléchargé l’artefact final dans `~/Downloads` et monté le DMG :

```bash
codesign --verify --deep --strict --verbose=4 "/Volumes/La Minute/La Minute.app"
codesign -dv --verbose=4 "/Volumes/La Minute/La Minute.app"
spctl --assess --type execute --verbose=4 "/Volumes/La Minute/La Minute.app"
xcrun stapler validate "/Volumes/La Minute/La Minute.app"
xcrun stapler validate "$HOME/Downloads/La.Minute_0.1.2_universal.dmg"
```

Le détail de signature doit contenir `Authority=Developer ID Application` et un `TeamIdentifier`. `spctl` doit accepter l’application et `stapler` doit valider les tickets.

Pour tester le premier consentement microphone sur une machine de recette, réinitialiser uniquement l’autorisation de La Minute avant d’ouvrir l’application :

```bash
tccutil reset Microphone app.laminute.desktop
```

Le dialogue macOS ne doit pas apparaître au lancement. Il doit apparaître une seule fois après la confirmation du premier enregistrement, puis ne plus revenir aux lancements suivants.

Si macOS affiche précisément « Apple ne peut pas vérifier que cette app ne contient pas de logiciel malveillant », vérifier d’abord la signature et le ticket avec les commandes ci-dessus. Si le message affirme que l’application « endommagera votre ordinateur », conserver une capture exacte : il peut s’agir d’un blocage XProtect distinct d’une simple absence de notarisation.
