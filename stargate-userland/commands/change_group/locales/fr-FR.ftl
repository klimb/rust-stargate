change_group-about = Changer le groupe de chaque FICHIER vers GROUPE.
change_group-usage = chgrp [OPTION]... GROUPE FICHIER...
  chgrp [OPTION]... --reference=RFICHIER FICHIER...

# Messages d'aide
change_group-help-print-help = Afficher les informations d'aide.
change_group-help-changes = comme verbeux mais rapporter seulement lors d'un changement
change_group-help-quiet = supprimer la plupart des messages d'erreur
change_group-help-verbose = afficher un diagnostic pour chaque fichier traité
change_group-help-preserve-root = échouer à opérer récursivement sur '/'
change_group-help-no-preserve-root = ne pas traiter '/' spécialement (par défaut)
change_group-help-reference = utiliser le groupe de RFICHIER plutôt que spécifier les valeurs de GROUPE
change_group-help-from = changer le groupe seulement si son groupe actuel correspond à GROUPE
change_group-help-recursive = opérer sur les fichiers et répertoires récursivement

# Messages d'erreur
change_group-error-invalid-group-id = identifiant de groupe invalide : '{ $gid_str }'
change_group-error-invalid-group = groupe invalide : '{ $group }'
change_group-error-failed-to-get-attributes = échec de l'obtention des attributs de { $file }
change_group-error-invalid-user = utilisateur invalide : '{ $from_group }'
