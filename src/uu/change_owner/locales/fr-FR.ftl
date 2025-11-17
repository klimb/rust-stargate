change_owner-about = Changer le propriétaire et le groupe des fichiers
change_owner-usage = change-owner [OPTION]... [PROPRIÉTAIRE][:[GROUPE]] FICHIER...
  change-owner [OPTION]... --reference=RFICHIER FICHIER...

# Messages d'aide
change_owner-help-print-help = Afficher les informations d'aide.
change_owner-help-changes = comme verbeux mais rapporter seulement lors d'un changement
change_owner-help-from = changer le propriétaire et/ou le groupe de chaque fichier seulement si son
  propriétaire et/ou groupe actuel correspondent à ceux spécifiés ici.
  L'un ou l'autre peut être omis, auquel cas une correspondance n'est pas requise
  pour l'attribut omis
change_owner-help-preserve-root = échouer à opérer récursivement sur '/'
change_owner-help-no-preserve-root = ne pas traiter '/' spécialement (par défaut)
change_owner-help-quiet = supprimer la plupart des messages d'erreur
change_owner-help-recursive = opérer sur les fichiers et répertoires récursivement
change_owner-help-reference = utiliser le propriétaire et groupe de RFICHIER plutôt que spécifier les valeurs PROPRIÉTAIRE:GROUPE
change_owner-help-verbose = afficher un diagnostic pour chaque fichier traité

# Messages d'erreur
change_owner-error-failed-to-get-attributes = échec de l'obtention des attributs de { $file }
change_owner-error-invalid-user = utilisateur invalide : { $user }
change_owner-error-invalid-group = groupe invalide : { $group }
change_owner-error-invalid-spec = spécification invalide : { $spec }