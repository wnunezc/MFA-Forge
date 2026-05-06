# Guide Utilisateur MFA-Forge

## Vue d'ensemble
MFA-Forge est un gestionnaire MFA local-first pour Windows. Son objectif principal est de conserver les comptes TOTP dans un coffre chiffre tout en offrant un flux coherent entre la GUI, la CLI humaine, la session d'agent local et le serveur MCP. L'application cherche a garder les secrets sur la machine locale, a rendre les actions sensibles explicites et a empecher l'automatisation de contourner silencieusement les limites d'unlock et de grants.

## Demarrage
Au premier lancement, vous devez creer un mot de passe maitre. Ce mot de passe est la cle principale du coffre: sans lui, vous ne pouvez pas ajouter de comptes, importer des seeds, exporter des sauvegardes, faire une rotation du mot de passe ou generer des codes.

Apres la saisie du mot de passe maitre, MFA-Forge execute encore la verification Windows supplementaire utilisee par cette ligne de release. En pratique, l'application n'est utilisable que lorsque les deux etapes reussissent.

Une fois deverrouillee, la fenetre principale se divise en trois zones:

- l'arbre des workspaces a gauche
- la liste des comptes au centre
- les actions contextuelles et les dialogues par-dessus cette mise en page

L'idee est de choisir d'abord le contexte, puis d'agir dans ce perimetre sans changer d'ecran.

## Workspaces
Les workspaces sont le systeme de regroupement des comptes. Utilisez-les pour separer les tokens par projet, client, environnement ou equipe.

Fonctionnement:

- un workspace racine est le conteneur principal
- un sous-repertoire est un chemin imbrique sous un workspace existant
- un compte peut appartenir a un chemin de workspace ou rester non assigne

Pourquoi c'est utile:

- le workspace actif filtre la vue des comptes
- les nouveaux comptes heritent par defaut du workspace selectionne
- l'export, la restauration et la revue deviennent plus simples si les comptes sont ranges de maniere coherente

Pour des comptes personnels ou de secours, les laisser hors workspace peut etre pratique afin de ne pas les melanger avec les projets.

## Ajout de comptes
MFA-Forge prend en charge quatre modes principaux pour charger un compte TOTP:

1. Saisie manuelle
2. Import d'un URI `otpauth://`
3. Import d'une image QR
4. Import d'un fichier compatible

La saisie manuelle convient quand vous voulez controler directement le service, l'utilisateur, le workspace, l'algorithme, les chiffres et la periode.

L'import par URI, QR ou fichier est preferable lorsqu'un autre systeme vous a deja fourni le seed au format TOTP standard. Dans ce cas, MFA-Forge analyse la source, extrait les champs du compte et enregistre le secret de facon chiffree dans le coffre.

Comportement important:

- les secrets restent masques dans l'interface
- les dialogues d'import nettoient le texte sensible a la fermeture
- modifier la metadata ne force pas le changement du secret
- modifier le secret est optionnel; si le champ reste vide, le secret chiffre existant est conserve

## Tokens et historique
La fenetre de token est la vue operationnelle pour lire un code. Quand vous l'ouvrez depuis une ligne de compte, MFA-Forge lit la valeur TOTP actuelle dans le coffre deverrouille et affiche le compte a rebours de la periode active.

Ce qu'il faut attendre au rafraichissement:

- si la meme periode TOTP est encore active, un rafraichissement peut renvoyer exactement le meme code
- si la periode a change, le code visible est mis a jour immediatement
- copier un code ne copie que le token courant, jamais le secret

L'historique sert a autre chose. Il n'est pas la pour lire des tokens, mais pour recuperer un etat precedent.

Le dialogue de restauration permet de:

- consulter les snapshots restaurables
- recuperer des comptes supprimes
- restaurer une version precedente visible dans le coffre actif

Utilisez l'historique lorsqu'un compte a ete supprime par erreur, lorsqu'une metadata a ete mal modifiee ou lorsque vous devez revenir a une version precedente sans recreer le compte manuellement.

## Sauvegarde et import
L'export cree une sauvegarde chiffree MFA-Forge. Son but est de preserver le coffre complet dans un format que MFA-Forge pourra reimporter plus tard.

L'import a volontairement un effet fort: apres validation, il remplace le contenu du coffre actif par la sauvegarde chiffree importee. C'est utile pour la reprise apres incident ou la migration de machine, mais cela doit etre traite comme une restauration controlee, pas comme une fusion.

Bonne pratique:

- creer une sauvegarde avant des changements importants ou des imports en masse
- stocker les sauvegardes dans un emplacement protege
- verifier que vous importez bien la sauvegarde attendue avant de l'appliquer

## Agent local et MCP
La session d'agent local et le serveur MCP existent pour l'automatisation locale, mais ils ne sont pas traites comme des canaux de confiance permanents.

Comportement de base:

- les deux demarrent en mode `deny-by-default`
- l'ouverture d'une session exige le flux natif d'unlock
- la session deverrouillee ne vit que tant que le processus reste actif
- les operations sensibles exigent des grants explicites ou des prompts dedies

Exemples d'actions protegees:

- generer un token pour un compte
- approvisionner ou importer des comptes
- lire un historique ou une audit trace sensible
- faire une rotation du mot de passe maitre

L'automatisation est donc possible, mais elle reste bornee par l'approbation explicite de l'utilisateur et par la duree de vie de la session locale.

## Depannage
Si l'unlock echoue:

- verifiez d'abord le mot de passe maitre
- terminez ensuite le prompt de verification Windows s'il apparait
- si l'application revient au loader, recommencez et surveillez une fenetre native en dehors de la fenetre principale

Si un import echoue:

- verifiez que la source contient toujours une charge `otpauth://` valide
- verifiez que le secret Base32 est complet
- verifiez que l'image QR selectionnee correspond bien au seed attendu

Si le token semble inchanger:

- regardez les secondes restantes de la periode TOTP en cours
- refaites un rafraichissement apres le changement de periode

Si une automatisation est refusee:

- verifiez si la session est encore ouverte
- verifiez si le grant requis a expire ou a deja ete consomme
- rouvrez la session locale et re-approuvez l'action exacte si necessaire
