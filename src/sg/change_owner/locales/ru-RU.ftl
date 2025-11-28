change_owner-about = Изменить владельца и группу файла
change_owner-usage = change-owner [ПАРАМЕТР]... [ВЛАДЕЛЕЦ][:[ГРУППА]] ФАЙЛ...
change_owner-usage = change-owner [ПАРАМЕТР]... --reference=RFILE ФАЙЛ...
change_owner-help-changes = аналогично подробному, но сообщать только при внесении изменений
change_owner-help-from = изменять владельца и/или группу каждого файла только в том случае, если его текущий владелец и/или группа соответствуют указанным здесь. Любой из параметров может быть опущен, в этом случае совпадение для опущенного атрибута не требуется
change_owner-help-preserve-root = не выполнять рекурсивную операцию над '/'
change_owner-help-no-preserve-root = не рассматривать '/' особым образом (по умолчанию)
change_owner-help-quiet = подавлять большинство сообщений об ошибках
change_owner-help-recursive = выполнять операции над файлами и каталогами рекурсивно
change_owner-help-reference = использовать владельца и группу RFILE вместо указания значений ВЛАДЕЛЕЦ:ГРУППА
change_owner-help-verbose = выводить диагностику для каждого обрабатываемого файла
change_owner-error-failed-to-get-attributes = не удалось получить атрибуты { $file }
change_owner-error-invalid-user = недопустимый пользователь: { $user }
change_owner-error-invalid-group = недопустимая группа: { $group }
change_owner-error-invalid-spec = недопустимая спецификация: { $spec }