pub fn execute(args: &[String]) -> Result<String, String> {
    let path = if args.is_empty() {
        std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
    } else if args[0] == "-" {
        std::env::var("OLDPWD").unwrap_or_else(|_| ".".to_string())
    } else {
        args[0].clone()
    };

    if let Ok(current) = std::env::current_dir() {
        unsafe { std::env::set_var("OLDPWD", current); }
    }

    std::env::set_current_dir(&path)
        .map_err(|e| format!("cd: {}: {}", path, e))?;

    if let Ok(new_dir) = std::env::current_dir() {
        unsafe { std::env::set_var("PWD", new_dir); }
    }

    Ok(String::new())
}
