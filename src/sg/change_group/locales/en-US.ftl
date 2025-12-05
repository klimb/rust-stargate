change_group-about = Change the group of each FILE to GROUP.
change_group-usage = change-group [OPTION]... GROUP FILE...
  change-group [OPTION]... --reference=RFILE FILE...

# Help messages
change_group-help-print-help = Print help information.
change_group-help-changes = like verbose but report only when a change is made
change_group-help-quiet = suppress most error messages
change_group-help-verbose = output a diagnostic for every file processed
change_group-help-preserve-root = fail to operate recursively on '/'
change_group-help-no-preserve-root = do not treat '/' specially (the default)
change_group-help-reference = use RFILE's group rather than specifying GROUP values
change_group-help-from = change the group only if its current group matches GROUP
change_group-help-recursive = operate on files and directories recursively

# Error messages
change_group-error-invalid-group-id = invalid group id: '{ $gid_str }'
change_group-error-invalid-group = invalid group: '{ $group }'
change_group-error-failed-to-get-attributes = failed to get attributes of { $file }
change_group-error-invalid-user = invalid user: '{ $from_group }'
