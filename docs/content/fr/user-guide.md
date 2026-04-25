# Guide Utilisateur

## Utilisation de base

1. Commencez par exporter votre avatar depuis VRoid Studio. À cette étape, sélectionnez **VRM1.0** comme format. Il est recommandé de réduire le nombre de polygones, mais vous n'avez pas à vous soucier de la taille des textures, car cet outil les redimensionne automatiquement à 1024.
   ![vroid_export](../images/vroid_export.avif)
2. Lancez cet outil, cliquez sur l'icône de dossier, puis sélectionnez le fichier \*.vrm exporté.
3. Après un court instant, l'avatar apparaît dans la zone de prévisualisation. Cliquez sur le bouton d'exportation et enregistrez le fichier \*.glb à l'emplacement de votre choix.
   ![vrm2sl](../images/vrm2sl.avif)
4. Ouvrez le viewer Second Life, puis sélectionnez Monde > Téléverser > Maillage. Quand la boîte de dialogue de sélection de fichier apparaît, choisissez le fichier \*.glb exporté précédemment.
   ![upload_mesh_model](../images/upload_mesh_model.avif)
5. La fenêtre de téléversement du modèle apparaît. Dans l'onglet Niveau de détail (LoD), définissez les valeurs LoD Moyen, Faible et Minimum à 0. Activez également Générer les normales.
   ::alert{type="info"}
   Ce n'est pas propre aux avatars, mais dans Second Life il est recommandé de définir les LoD Moyen, Faible et Minimum à 0. Cela réduit l'impact sur le terrain et la complexité, et peut aussi réduire le coût de téléversement.
   ::
6. Ensuite, allez dans l'onglet Options de téléversement et cochez Inclure les textures.
   ![upload_model-upload-options](../images/upload_model-upload-options.avif)
7. Allez dans l'onglet Rigging et cochez toutes les cases affichées.
   ![upload_model-rigging](../images/upload_model-rigging.avif)
   ::alert{type="warning"}
   Si ces cases ne sont pas cochées, le modèle ne pourra pas être utilisé comme avatar.
   ::
8. Une fois prêt, cliquez sur Calculer le coût pour estimer les frais, puis téléversez le modèle.
9. Faites un clic droit sur votre avatar, choisissez Enlever > Tout enlever, puis double-cliquez sur le maillage téléversé dans votre inventaire pour l'équiper.
   ::alert{type="info"}
   Vous devez préparer un alpha corps entier à l'avance. Cela masque le corps de l'avatar système.
   ::
10. Après avoir équipé le modèle téléversé, un problème d'alpha peut apparaître comme sur l'illustration. Avec l'avatar toujours équipé, faites un clic droit sur l'objet attaché dans l'inventaire puis cliquez sur Modifier.
    ![edit_mesh](../images/edit_mesh.avif)
11. Dans l'outil de build, ouvrez l'onglet Matériau. (Sur Firestorm, il peut être nécessaire d'ouvrir aussi l'onglet Blinn-Phong.) Réglez le mode Alpha sur Alpha Mask et la coupure du masque autour de 127. Cela corrige le problème d'alpha.
    ![build_tool_panel](../images/build_tool_panel.avif)

## Utilisation en ligne de commande

```bash
vrm2sl input.vrm output.glb --analyze-only --report report.json
```

## Remarques

Si vous comptez distribuer au public ou revendre sur une marketplace, vérifiez impérativement la **licence**. De nombreux avatars vendus ou distribués sur [Booth](https://booth.pm/fr/browse/VRoid), [Vroid Hub](https://hub.vroid.com/), [Niconico Commons](https://commons.nicovideo.jp/search?keywords=VROID&sort=created&order=desc) et [Etsy](https://www.etsy.com/fr/search?q=Vroid&ref=search_bar) **interdisent l'utilisation dérivée, la redistribution ou la revente**.
