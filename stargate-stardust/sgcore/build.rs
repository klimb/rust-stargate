use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out_dir = env::var("OUT_DIR")?;

    let mut embedded_file = File::create(Path::new(&out_dir).join("embedded_locales.rs"))?;
    writeln!(embedded_file, "// Generated at compile time - do not edit")?;
    writeln!(
        embedded_file,
        "// This file contains embedded English locale files"
    )?;
    writeln!(embedded_file)?;
    writeln!(embedded_file)?;

    writeln!(
        embedded_file,
        "pub fn get_embedded_locale(key: &str) -> Option<&'static str> {{"
    )?;
    writeln!(embedded_file, "    match key {{")?;

    let target_utility = detect_target_utility();
    let locales_to_embed = get_locales_to_embed();

    match target_utility {
        Some(util_name) => {
            embed_single_utility_locale(
                &mut embedded_file,
                &project_root()?,
                &util_name,
                &locales_to_embed
            )?;
        }
        None => {
            embed_all_utility_locales(&mut embedded_file, &project_root()?, &locales_to_embed)?;
        }
    }

    writeln!(embedded_file, "        _ => None,")?;
    writeln!(embedded_file, "    }}")?;
    writeln!(embedded_file, "}}")?;

    embedded_file.flush()?;
    Ok(())
}

fn project_root() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")?;
    let uucore_path = std::path::Path::new(&manifest_dir);

    let project_root = uucore_path
        .parent()
        .and_then(|p| p.parent())
        .ok_or("Could not determine project root")?;

    Ok(project_root.to_path_buf())
}

fn detect_target_utility() -> Option<String> {
    use std::fs;

    println!("cargo:rerun-if-env-changed=UUCORE_TARGET_UTIL");

    if let Ok(target_util) = env::var("UUCORE_TARGET_UTIL") {
        if !target_util.is_empty() {
            return Some(target_util);
        }
    }

    if let Ok(pkg_name) = env::var("CARGO_PKG_NAME") {
        if let Some(util_name) = pkg_name.strip_prefix("sg_") {
            println!("cargo:warning=Auto-detected utility name: {util_name}");
            return Some(util_name.to_string());
        }
    }

    if let Ok(target_dir) = env::var("CARGO_TARGET_DIR") {
        let config_path = std::path::Path::new(&target_dir).join("uucore_target_util.txt");
        if let Ok(content) = fs::read_to_string(&config_path) {
            let util_name = content.trim();
            if !util_name.is_empty() && util_name != "multicall" {
                return Some(util_name.to_string());
            }
        }
    }

    if let Ok(project_root) = project_root() {
        let config_path = project_root.join("target/uucore_target_util.txt");
        if let Ok(content) = fs::read_to_string(&config_path) {
            let util_name = content.trim();
            if !util_name.is_empty() && util_name != "multicall" {
                return Some(util_name.to_string());
            }
        }
    }

    None
}

fn embed_single_utility_locale(
    embedded_file: &mut std::fs::File,
    project_root: &Path,
    util_name: &str,
    locales_to_embed: &(String, Option<String>)
) -> Result<(), Box<dyn std::error::Error>> {
    embed_component_locales(embedded_file, locales_to_embed, util_name, |locale| {
        project_root
            .join("stargate-stardust/commands")
            .join(util_name)
            .join(format!("locales/{locale}.ftl"))
    })?;

    embed_component_locales(embedded_file, locales_to_embed, "sgcore", |locale| {
        project_root.join(format!("stargate-stardust/sgcore/locales/{locale}.ftl"))
    })?;

    Ok(())
}

fn embed_all_utility_locales(
    embedded_file: &mut std::fs::File,
    project_root: &Path,
    locales_to_embed: &(String, Option<String>)
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    let commands_dir = project_root.join("stargate-stardust/commands");
    if !commands_dir.exists() {
        embed_static_utility_locales(embedded_file, locales_to_embed)?;
        return Ok(());
    }

    let mut util_dirs = Vec::new();
    for entry in fs::read_dir(&commands_dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            if let Some(dir_name) = entry.file_name().to_str() {
                util_dirs.push(dir_name.to_string());
            }
        }
    }
    util_dirs.sort();

    for util_name in &util_dirs {
        embed_component_locales(embedded_file, locales_to_embed, util_name, |locale| {
            commands_dir
                .join(util_name)
                .join(format!("locales/{locale}.ftl"))
        })?;
    }

    embed_component_locales(embedded_file, locales_to_embed, "sgcore", |locale| {
        project_root.join(format!("stargate-stardust/sgcore/locales/{locale}.ftl"))
    })?;

    embedded_file.flush()?;
    Ok(())
}

fn embed_static_utility_locales(
    embedded_file: &mut std::fs::File,
    locales_to_embed: &(String, Option<String>)
) -> Result<(), Box<dyn std::error::Error>> {
    use std::env;

    writeln!(
        embedded_file,
        "        // Static utility locales for crates.io builds"
    )?;

    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let Some(registry_dir) = Path::new(&manifest_dir).parent() else {
        return Ok(());
    };

    embed_component_locales(embedded_file, locales_to_embed, "sgcore", |locale| {
        Path::new(&manifest_dir).join(format!("locales/{locale}.ftl"))
    })?;

    let mut entries: Vec<_> = std::fs::read_dir(registry_dir)?
        .filter_map(Result::ok)
        .collect();
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let file_name = entry.file_name();
        if let Some(dir_name) = file_name.to_str() {
            if let Some((util_part, _)) = dir_name.split_once('-') {
                if let Some(util_name) = util_part.strip_prefix("sg_") {
                    embed_component_locales(
                        embedded_file,
                        locales_to_embed,
                        util_name,
                        |locale| entry.path().join(format!("locales/{locale}.ftl"))
                    )?;
                }
            }
        }
    }

    Ok(())
}

fn get_locales_to_embed() -> (String, Option<String>) {
    let system_locale = env::var("LANG").ok().and_then(|lang| {
        let locale = lang.split('.').next()?.replace('_', "-");
        if locale != "en-US" && !locale.is_empty() {
            Some(locale)
        } else {
            None
        }
    });
    ("en-US".to_string(), system_locale)
}

fn for_each_locale<F>(
    locales: &(String, Option<String>),
    mut f: F
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(&str) -> Result<(), Box<dyn std::error::Error>>,
{
    f(&locales.0)?;
    if let Some(ref system_locale) = locales.1 {
        f(system_locale)?;
    }
    Ok(())
}

fn embed_locale_file(
    embedded_file: &mut std::fs::File,
    locale_path: &Path,
    locale_key: &str,
    locale: &str,
    component: &str
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;

    if locale_path.exists() || locale_path.is_file() {
        let content = fs::read_to_string(locale_path)?;
        writeln!(
            embedded_file,
            "        // Locale for {component} ({locale})"
        )?;
        writeln!(
            embedded_file,
            "        \"{locale_key}\" => Some(r###\"{content}\"###),"
        )?;

        println!("cargo:rerun-if-changed={}", locale_path.display());
    }
    Ok(())
}

fn embed_component_locales<F>(
    embedded_file: &mut std::fs::File,
    locales: &(String, Option<String>),
    component_name: &str,
    path_builder: F
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Fn(&str) -> std::path::PathBuf,
{
    for_each_locale(locales, |locale| {
        let locale_path = path_builder(locale);
        embed_locale_file(
            embedded_file,
            &locale_path,
            &format!("{component_name}/{locale}.ftl"),
            locale,
            component_name
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_locales_to_embed_no_lang() {
        unsafe {
            env::remove_var("LANG");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, None);

        unsafe {
            env::set_var("LANG", "");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, None);
        unsafe {
            env::remove_var("LANG");
        }

        unsafe {
            env::set_var("LANG", "en_US.UTF-8");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, None);
        unsafe {
            env::remove_var("LANG");
        }
    }

    #[test]
    fn get_locales_to_embed_with_lang() {
        unsafe {
            env::set_var("LANG", "fr_FR.UTF-8");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("fr-FR".to_string()));
        unsafe {
            env::remove_var("LANG");
        }

        unsafe {
            env::set_var("LANG", "zh_CN.UTF-8");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("zh-CN".to_string()));
        unsafe {
            env::remove_var("LANG");
        }

        unsafe {
            env::set_var("LANG", "de");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("de".to_string()));
        unsafe {
            env::remove_var("LANG");
        }
    }

    #[test]
    fn get_locales_to_embed_invalid_lang() {
        // invalid locale format
        unsafe {
            env::set_var("LANG", "invalid");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("invalid".to_string()));
        unsafe {
            env::remove_var("LANG");
        }

        // numeric values
        unsafe {
            env::set_var("LANG", "123");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("123".to_string()));
        unsafe {
            env::remove_var("LANG");
        }

        // special characters
        unsafe {
            env::set_var("LANG", "@@@@");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("@@@@".to_string()));
        unsafe {
            env::remove_var("LANG");
        }

        // malformed locale (no country code but with encoding)
        unsafe {
            env::set_var("LANG", "en.UTF-8");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("en".to_string()));
        unsafe {
            env::remove_var("LANG");
        }

        // valid format but unusual locale
        unsafe {
            env::set_var("LANG", "XX_YY.UTF-8");
        }
        let (en_locale, system_locale) = get_locales_to_embed();
        assert_eq!(en_locale, "en-US");
        assert_eq!(system_locale, Some("XX-YY".to_string()));
        unsafe {
            env::remove_var("LANG");
        }
    }

    #[test]
    fn for_each_locale_basic() {
        let locales = ("en-US".to_string(), Some("fr-FR".to_string()));
        let mut collected = Vec::new();

        for_each_locale(&locales, |locale| {
            collected.push(locale.to_string());
            Ok(())
        })
        .unwrap();

        assert_eq!(collected, vec!["en-US", "fr-FR"]);
    }

    #[test]
    fn for_each_locale_no_system_locale() {
        let locales = ("en-US".to_string(), None);
        let mut collected = Vec::new();

        for_each_locale(&locales, |locale| {
            collected.push(locale.to_string());
            Ok(())
        })
        .unwrap();

        assert_eq!(collected, vec!["en-US"]);
    }

    #[test]
    fn for_each_locale_error_handling() {
        let locales = ("en-US".to_string(), Some("fr-FR".to_string()));

        let result = for_each_locale(&locales, |_locale| Err("test error".into()));

        assert!(result.is_err());
    }
}
