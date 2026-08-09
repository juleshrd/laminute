# Publier La Minute sur Windows

## État actuel

La CI produit un installateur x64 `.exe` au format NSIS. Il est publiable et installable sans certificat payant, mais il n’est actuellement pas signé avec Authenticode.

La signature Minisign de l’updater reste obligatoire : elle permet à La Minute de vérifier qu’une mise à jour provient du projet. Windows et SmartScreen ne la reconnaissent toutefois pas comme une identité d’éditeur.

## Avertissement attendu

Microsoft Defender SmartScreen peut afficher **« Windows a protégé votre ordinateur »** et **« Éditeur inconnu »**. Selon la configuration du poste, l’utilisateur peut ouvrir **Informations complémentaires > Exécuter quand même**. Smart App Control ou une politique d’entreprise peuvent supprimer cette possibilité et bloquer totalement un EXE non signé.

Chaque nouvelle version non signée repart avec une réputation de fichier nulle. Une signature Authenticode stable permet de construire une réputation d’éditeur, mais même un nouveau certificat OV ou EV peut encore afficher SmartScreen au début. Microsoft indique que l’EV ne supprime plus automatiquement cet avertissement.

Référence : [réputation SmartScreen](https://learn.microsoft.com/fr-fr/windows/apps/package-and-deploy/smartscreen-reputation).

## Vérification

Depuis PowerShell :

```powershell
Get-AuthenticodeSignature .\La.Minute_0.1.2_x64-setup.exe | Format-List
```

Le résultat actuel attendu est différent de `Valid`. Le workflow affiche ce statut comme avertissement sans bloquer la release.

## Options sans budget

1. Continuer à publier le NSIS non signé avec l’avertissement transparent ci-dessus.
2. Candidater à [SignPath Foundation](https://signpath.org/), qui propose une signature gratuite à certains projets open source. L’acceptation n’est pas automatique.
3. Préparer un package MSIX pour le Microsoft Store. L’ouverture d’un compte développeur individuel est désormais gratuite et un MSIX distribué par le Store est signé par Microsoft, ce qui évite l’avertissement de téléchargement SmartScreen. Le dépôt ne produit pas encore de MSIX : cette option nécessite un chantier dédié.

Un simple EXE/NSIS envoyé au Store doit toujours être signé par l’éditeur. Voir les [options officielles de signature Windows](https://learn.microsoft.com/en-us/windows/apps/package-and-deploy/code-signing-options).

## Options payantes futures

- **Microsoft Artifact Signing** : service cloud adapté à la CI, environ 9,99 USD par mois pour le niveau de base ; disponibilité soumise au pays et au type de compte.
- **Certificat OV traditionnel** : certificat fourni par une autorité reconnue, généralement associé à une clé protégée par matériel ou service HSM.
- **EV** : plus cher et sans avantage automatique supplémentaire contre SmartScreen depuis 2024.

Tauri accepte un certificat importé dans le magasin Windows ou une commande de signature externe via `bundle.windows.signCommand`. Voir la [documentation Tauri Windows](https://v2.tauri.app/distribute/sign/windows/).
