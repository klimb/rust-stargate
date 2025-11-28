change_owner-about = Change file owner and group
change_owner-usage = change-owner [OPTION]... [OWNER][:[GROUP]] FILE...
  change-owner [OPTION]... --reference=RFILE FILE...

# Help messages
change_owner-help-print-help = Print help information.
change_owner-help-changes = like verbose but report only when a change is made
change_owner-help-from = change the owner and/or group of each file only if its
  current owner and/or group match those specified here.
  Either may be omitted, in which case a match is not required
  for the omitted attribute
change_owner-help-preserve-root = fail to operate recursively on '/'
change_owner-help-no-preserve-root = do not treat '/' specially (the default)
change_owner-help-quiet = suppress most error messages
change_owner-help-recursive = operate on files and directories recursively
change_owner-help-reference = use RFILE's owner and group rather than specifying OWNER:GROUP values
change_owner-help-verbose = output a diagnostic for every file processed

# Error messages
change_owner-error-failed-to-get-attributes = failed to get attributes of { $file }
change_owner-error-invalid-user = invalid user: { $user }
change_owner-error-invalid-group = invalid group: { $group }
change_owner-error-invalid-spec = invalid spec: { $spec }