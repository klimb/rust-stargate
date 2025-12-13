
use crate::error::SGError;

use fluent::{FluentArgs, FluentBundle, FluentResource};
use fluent_syntax::parser::ParserError;

use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;

use thiserror::Error;
use unic_langid::LanguageIdentifier;

#[derive(Error, Debug)]
pub enum LocalizationError {
    #[error("I/O error loading '{path}': {source}")]
    Io {
        source: std::io::Error,
        path: PathBuf,
    },
    #[error("Parse-locale error: {0}")]
    ParseLocale(String),
    #[error("Resource parse error at '{snippet}': {error:?}")]
    ParseResource {
        #[source]
        error: ParserError,
        snippet: String,
    },
    #[error("Bundle error: {0}")]
    Bundle(String),
    #[error("Locales directory not found: {0}")]
    LocalesDirNotFound(String),
    #[error("Path resolution error: {0}")]
    PathResolution(String),
}

impl From<std::io::Error> for LocalizationError {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            source: error,
            path: PathBuf::from("<unknown>"),
        }
    }
}

impl SGError for LocalizationError {
    fn code(&self) -> i32 {
        1
    }
}

pub const DEFAULT_LOCALE: &str = "en-US";

include!(concat!(env!("OUT_DIR"), "/embedded_locales.rs"));

struct Localizer {
    primary_bundle: FluentBundle<FluentResource>,
    fallback_bundle: Option<FluentBundle<FluentResource>>,
}

impl Localizer {
    fn new(primary_bundle: FluentBundle<FluentResource>) -> Self {
        Self {
            primary_bundle,
            fallback_bundle: None,
        }
    }

    fn with_fallback(mut self, fallback_bundle: FluentBundle<FluentResource>) -> Self {
        self.fallback_bundle = Some(fallback_bundle);
        self
    }

    fn format(&self, id: &str, args: Option<&FluentArgs>) -> String {

        if let Some(message) = self.primary_bundle.get_message(id).and_then(|m| m.value()) {
            let mut errs = Vec::new();
            return self
                .primary_bundle
                .format_pattern(message, args, &mut errs)
                .to_string();
        }

        if let Some(ref fallback) = self.fallback_bundle {
            if let Some(message) = fallback.get_message(id).and_then(|m| m.value()) {
                let mut errs = Vec::new();
                return fallback
                    .format_pattern(message, args, &mut errs)
                    .to_string();
            }
        }

        id.to_string()
    }
}

thread_local! {
    static LOCALIZER: OnceLock<Localizer> = const { OnceLock::new() };
}

fn find_uucore_locales_dir(utility_locales_dir: &Path) -> Option<PathBuf> {

    let normalized_dir = utility_locales_dir
        .canonicalize()
        .unwrap_or_else(|_| utility_locales_dir.to_path_buf());

    if let Some(uucore_locales) = normalized_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.join("..").join("sgcore").join("locales"))
        .and_then(|p| p.canonicalize().ok())
    {
        if uucore_locales.exists() {
            return Some(uucore_locales);
        }
    }

    let uucore_locales = normalized_dir
        .parent()?
        .parent()?
        .parent()?
        .join("sgcore")
        .join("locales");

    uucore_locales.exists().then_some(uucore_locales)
}

fn create_bundle(
    locale: &LanguageIdentifier,
    locales_dir: &Path,
    util_name: &str
) -> Result<FluentBundle<FluentResource>, LocalizationError> {
    let mut bundle = FluentBundle::new(vec![locale.clone()]);

    bundle.set_use_isolating(false);

    let mut try_add_resource_from = |dir_opt: Option<std::path::PathBuf>| {
        if let Some(resource) = dir_opt
            .map(|dir| dir.join(format!("{locale}.ftl")))
            .and_then(|locale_path| fs::read_to_string(locale_path).ok())
            .and_then(|ftl| fluent_bundle::FluentResource::try_new(ftl).ok())
        {
            bundle.add_resource_overriding(resource);
        }
    };

    try_add_resource_from(find_uucore_locales_dir(locales_dir));

    try_add_resource_from(get_locales_dir(util_name).ok());

    if bundle.has_message("common-error") || bundle.has_message(&format!("{util_name}-about")) {
        Ok(bundle)
    } else {
        Err(LocalizationError::LocalesDirNotFound(format!(
            "No localization strings found for {locale} and utility {util_name}"
        )))
    }
}

fn init_localization(
    locale: &LanguageIdentifier,
    locales_dir: &Path,
    util_name: &str
) -> Result<(), LocalizationError> {
    let default_locale = LanguageIdentifier::from_str(DEFAULT_LOCALE)
        .expect("Default locale should always be valid");

    let english_bundle = create_bundle(&default_locale, locales_dir, util_name).or_else(|_| {

        create_english_bundle_from_embedded(&default_locale, util_name)
    })?;

    let loc = if locale == &default_locale {

        Localizer::new(english_bundle)
    } else {

        if let Ok(primary_bundle) = create_bundle(locale, locales_dir, util_name) {

            Localizer::new(primary_bundle).with_fallback(english_bundle)
        } else {

            Localizer::new(english_bundle)
        }
    };

    LOCALIZER.with(|lock| {
        lock.set(loc)
            .map_err(|_| LocalizationError::Bundle("Localizer already initialized".into()))
    })?;
    Ok(())
}

fn parse_fluent_resource(content: &str) -> Result<FluentResource, LocalizationError> {
    FluentResource::try_new(content.to_string()).map_err(
        |(_partial_resource, errs): (FluentResource, Vec<ParserError>)| {
            if let Some(first_err) = errs.into_iter().next() {
                let snippet = first_err
                    .slice
                    .clone()
                    .and_then(|range| content.get(range))
                    .unwrap_or("")
                    .to_string();
                LocalizationError::ParseResource {
                    error: first_err,
                    snippet,
                }
            } else {
                LocalizationError::LocalesDirNotFound("Parse error without details".to_string())
            }
        }
    )
}

fn create_english_bundle_from_embedded(
    locale: &LanguageIdentifier,
    util_name: &str
) -> Result<FluentBundle<FluentResource>, LocalizationError> {

    if *locale != "en-US" {
        return Err(LocalizationError::LocalesDirNotFound(
            "Embedded locales only support en-US".to_string()
        ));
    }

    let mut bundle = FluentBundle::new(vec![locale.clone()]);
    bundle.set_use_isolating(false);

    if let Some(uucore_content) = get_embedded_locale("sgcore/en-US.ftl") {
        let uucore_resource = parse_fluent_resource(uucore_content)?;
        bundle.add_resource_overriding(uucore_resource);
    }

    let locale_key = format!("{util_name}/en-US.ftl");
    if let Some(ftl_content) = get_embedded_locale(&locale_key) {
        let resource = parse_fluent_resource(ftl_content)?;
        bundle.add_resource_overriding(resource);
    }

    if bundle.has_message("common-error") || bundle.has_message(&format!("{util_name}-about")) {
        Ok(bundle)
    } else {
        Err(LocalizationError::LocalesDirNotFound(format!(
            "No embedded locale found for {util_name} and no common strings found"
        )))
    }
}

fn get_message_internal(id: &str, args: Option<FluentArgs>) -> String {
    LOCALIZER.with(|lock| {
        lock.get()
            .map(|loc| loc.format(id, args.as_ref()))
            .unwrap_or_else(|| id.to_string())
    })
}

pub fn get_message(id: &str) -> String {
    get_message_internal(id, None)
}

pub fn get_message_with_args(id: &str, ftl_args: FluentArgs) -> String {
    get_message_internal(id, Some(ftl_args))
}

fn detect_system_locale() -> Result<LanguageIdentifier, LocalizationError> {

    let locale_str = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_else(|_| DEFAULT_LOCALE.to_string())
        .split('.')
        .next()
        .unwrap_or(DEFAULT_LOCALE)
        .to_string();

    let locale_str = if locale_str == "C" || locale_str == "POSIX" {
        DEFAULT_LOCALE.to_string()
    } else {
        locale_str
    };

    LanguageIdentifier::from_str(&locale_str).map_err(|_| {
        LocalizationError::ParseLocale(format!("Failed to parse locale: {locale_str}"))
    })
}

pub fn setup_localization(p: &str) -> Result<(), LocalizationError> {
    let locale = detect_system_locale().unwrap_or_else(|_| {
        LanguageIdentifier::from_str(DEFAULT_LOCALE).expect("Default locale should always be valid")
    });

    match get_locales_dir(p) {
        Ok(locales_dir) => {

            init_localization(&locale, &locales_dir, p)
        }
        Err(_) => {

            let default_locale = LanguageIdentifier::from_str(DEFAULT_LOCALE)
                .expect("Default locale should always be valid");
            let english_bundle = create_english_bundle_from_embedded(&default_locale, p)?;
            let localizer = Localizer::new(english_bundle);

            LOCALIZER.with(|lock| {
                lock.set(localizer)
                    .map_err(|_| LocalizationError::Bundle("Localizer already initialized".into()))
            })?;
            Ok(())
        }
    }
}

#[cfg(not(debug_assertions))]
fn resolve_locales_dir_from_exe_dir(exe_dir: &Path, p: &str) -> Option<PathBuf> {

    let coreutils = exe_dir.join("locales").join(p);
    if coreutils.exists() {
        return Some(coreutils);
    }

    if let Some(prefix) = exe_dir.parent() {
        let fhs = prefix.join("share").join("locales").join(p);
        if fhs.exists() {
            return Some(fhs);
        }
    }

    let fallback = exe_dir.join(p);
    if fallback.exists() {
        return Some(fallback);
    }

    None
}

fn get_locales_dir(p: &str) -> Result<PathBuf, LocalizationError> {

    let dir_name = p.replace('-', "_");

    #[cfg(debug_assertions)]
    {

        let manifest_dir = env!("CARGO_MANIFEST_DIR");

        let subdirs = ["text-commands", "stardust-commands", "stardust-native"];
        for subdir in &subdirs {
            let organized_path = PathBuf::from(manifest_dir)
                .join("../commands")
                .join(subdir)
                .join(&dir_name)
                .join("locales");

            if organized_path.exists() {
                return Ok(organized_path);
            }
        }

        let dev_path = PathBuf::from(manifest_dir)
            .join("../commands")
            .join(&dir_name)
            .join("locales");

        if dev_path.exists() {
            return Ok(dev_path);
        }

        let sg_path = PathBuf::from(manifest_dir)
            .join("../uu")
            .join(&dir_name)
            .join("locales");

        if sg_path.exists() {
            return Ok(sg_path);
        }

        let fallback_dev_path = PathBuf::from(manifest_dir).join(&dir_name);
        if fallback_dev_path.exists() {
            return Ok(fallback_dev_path);
        }

        Err(LocalizationError::LocalesDirNotFound(format!(
            "Development locales directory not found in organized structure or at {}, {}, or {}",
            dev_path.display(),
            sg_path.display(),
            fallback_dev_path.display()
        )))
    }

    #[cfg(not(debug_assertions))]
    {
        use std::env;

        let exe_path = env::current_exe().map_err(|e| {
            LocalizationError::PathResolution(format!("Failed to get executable path: {e}"))
        })?;

        let exe_dir = exe_path.parent().ok_or_else(|| {
            LocalizationError::PathResolution("Failed to get executable directory".to_string())
        })?;

        if let Some(dir) = resolve_locales_dir_from_exe_dir(exe_dir, p) {
            return Ok(dir);
        }

        Err(LocalizationError::LocalesDirNotFound(format!(
            "Release locales directory not found starting from {}",
            exe_dir.display()
        )))
    }
}

#[macro_export]
macro_rules! translate {

    ($id:expr) => {
        $crate::locale::get_message($id)
    };

    ($id:expr, $($key:expr => $value:expr),+ $(,)?) => {
        {
            let mut args = fluent::FluentArgs::new();
            $(
                let value_str = $value.to_string();
                if let Ok(num_val) = value_str.parse::<i64>() {
                    args.set($key, num_val);
                } else if let Ok(float_val) = value_str.parse::<f64>() {
                    args.set($key, float_val);
                } else {

                    args.set($key, value_str);
                }
            )+
            $crate::locale::get_message_with_args($id, args)
        }
    };
}

pub use translate;

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[cfg(test)]
    fn create_test_bundle(
        locale: &LanguageIdentifier,
        test_locales_dir: &Path
    ) -> Result<FluentBundle<FluentResource>, LocalizationError> {
        let mut bundle = FluentBundle::new(vec![locale.clone()]);
        bundle.set_use_isolating(false);

        let locale_path = test_locales_dir.join(format!("{locale}.ftl"));
        if let Ok(ftl_content) = fs::read_to_string(&locale_path) {
            let resource = parse_fluent_resource(&ftl_content)?;
            bundle.add_resource_overriding(resource);
            return Ok(bundle);
        }

        Err(LocalizationError::LocalesDirNotFound(format!(
            "No localization strings found for {locale} in {}",
            test_locales_dir.display()
        )))
    }

    #[cfg(test)]
    fn init_test_localization(
        locale: &LanguageIdentifier,
        test_locales_dir: &Path
    ) -> Result<(), LocalizationError> {
        let default_locale = LanguageIdentifier::from_str(DEFAULT_LOCALE)
            .expect("Default locale should always be valid");

        let english_bundle = create_test_bundle(&default_locale, test_locales_dir)?;

        let loc = if locale == &default_locale {

            Localizer::new(english_bundle)
        } else {

            if let Ok(primary_bundle) = create_test_bundle(locale, test_locales_dir) {

                Localizer::new(primary_bundle).with_fallback(english_bundle)
            } else {

                Localizer::new(english_bundle)
            }
        };

        LOCALIZER.with(|lock| {
            lock.set(loc)
                .map_err(|_| LocalizationError::Bundle("Localizer already initialized".into()))
        })?;
        Ok(())
    }

    fn create_test_locales_dir() -> TempDir {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");

        let en_content = r#"
greeting = Hello, world!
welcome = Welcome, { $name }!
count-items = You have { $count ->
    [one] { $count } item
   *[other] { $count } items
}
missing-in-other = This message only exists in English
"#;

        let fr_content = r#"
greeting = Bonjour, le monde!
welcome = Bienvenue, { $name }!
count-items = Vous avez { $count ->
    [one] { $count } élément
   *[other] { $count } éléments
}
"#;

        let ja_content = r#"
greeting = こんにちは、世界！
welcome = ようこそ、{ $name }さん！
count-items = { $count }個のアイテムがあります
"#;

        let ar_content = r#"
greeting = أهلاً بالعالم！
welcome = أهلاً وسهلاً، { $name }！
count-items = لديك { $count ->
    [zero] لا عناصر
    [one] عنصر واحد
    [two] عنصران
    [few] { $count } عناصر
   *[other] { $count } عنصر
}
"#;

        let es_invalid_content = r#"
greeting = Hola, mundo!
invalid-syntax = This is { $missing
"#;

        fs::write(temp_dir.path().join("en-US.ftl"), en_content)
            .expect("Failed to write en-US.ftl");
        fs::write(temp_dir.path().join("fr-FR.ftl"), fr_content)
            .expect("Failed to write fr-FR.ftl");
        fs::write(temp_dir.path().join("ja-JP.ftl"), ja_content)
            .expect("Failed to write ja-JP.ftl");
        fs::write(temp_dir.path().join("ar-SA.ftl"), ar_content)
            .expect("Failed to write ar-SA.ftl");
        fs::write(temp_dir.path().join("es-ES.ftl"), es_invalid_content)
            .expect("Failed to write es-ES.ftl");

        temp_dir
    }

    #[test]
    fn test_create_bundle_success() {
        let temp_dir = create_test_locales_dir();
        let locale = LanguageIdentifier::from_str("en-US").unwrap();

        let result = create_test_bundle(&locale, temp_dir.path());
        assert!(result.is_ok());

        let bundle = result.unwrap();
        assert!(bundle.get_message("greeting").is_some());
    }

    #[test]
    fn test_create_bundle_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let locale = LanguageIdentifier::from_str("de-DE").unwrap();

        let result = create_test_bundle(&locale, temp_dir.path());
        assert!(result.is_err());

        if let Err(LocalizationError::LocalesDirNotFound(_)) = result {

        } else {
            panic!("Expected LocalesDirNotFound error");
        }
    }

    #[test]
    fn test_create_bundle_invalid_syntax() {
        let temp_dir = create_test_locales_dir();
        let locale = LanguageIdentifier::from_str("es-ES").unwrap();

        let result = create_test_bundle(&locale, temp_dir.path());

        match result {
            Err(LocalizationError::ParseResource {
                error: _parser_err,
                snippet: _,
            }) => {

            }
            Ok(_) => {
                panic!("Expected ParseResource error, but bundle was created successfully");
            }
            Err(other) => {
                panic!("Expected ParseResource error, but got: {other:?}");
            }
        }
    }

    #[test]
    fn test_localizer_format_primary_bundle() {
        let temp_dir = create_test_locales_dir();
        let en_bundle = create_test_bundle(
            &LanguageIdentifier::from_str("en-US").unwrap(),
            temp_dir.path()
        )
        .unwrap();

        let localizer = Localizer::new(en_bundle);
        let result = localizer.format("greeting", None);
        assert_eq!(result, "Hello, world!");
    }

    #[test]
    fn test_localizer_format_with_args() {
        use fluent::FluentArgs;
        let temp_dir = create_test_locales_dir();
        let en_bundle = create_test_bundle(
            &LanguageIdentifier::from_str("en-US").unwrap(),
            temp_dir.path()
        )
        .unwrap();

        let localizer = Localizer::new(en_bundle);
        let mut args = FluentArgs::new();
        args.set("name", "Alice");

        let result = localizer.format("welcome", Some(&args));
        assert_eq!(result, "Welcome, Alice!");
    }

    #[test]
    fn test_localizer_fallback_to_english() {
        let temp_dir = create_test_locales_dir();
        let fr_bundle = create_test_bundle(
            &LanguageIdentifier::from_str("fr-FR").unwrap(),
            temp_dir.path()
        )
        .unwrap();
        let en_bundle = create_test_bundle(
            &LanguageIdentifier::from_str("en-US").unwrap(),
            temp_dir.path()
        )
        .unwrap();

        let localizer = Localizer::new(fr_bundle).with_fallback(en_bundle);

        let result1 = localizer.format("greeting", None);
        assert_eq!(result1, "Bonjour, le monde!");

        let result2 = localizer.format("missing-in-other", None);
        assert_eq!(result2, "This message only exists in English");
    }

    #[test]
    fn test_localizer_format_message_not_found() {
        let temp_dir = create_test_locales_dir();
        let en_bundle = create_test_bundle(
            &LanguageIdentifier::from_str("en-US").unwrap(),
            temp_dir.path()
        )
        .unwrap();

        let localizer = Localizer::new(en_bundle);
        let result = localizer.format("nonexistent-message", None);
        assert_eq!(result, "nonexistent-message");
    }

    #[test]
    fn test_init_localization_english_only() {

        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("en-US").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let message = get_message("greeting");
            assert_eq!(message, "Hello, world!");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_init_localization_with_fallback() {
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("fr-FR").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let message1 = get_message("greeting");
            assert_eq!(message1, "Bonjour, le monde!");

            let message2 = get_message("missing-in-other");
            assert_eq!(message2, "This message only exists in English");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_init_localization_invalid_locale_falls_back_to_english() {
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("de-DE").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let message = get_message("greeting");
            assert_eq!(message, "Hello, world!");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_init_localization_already_initialized() {
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("en-US").unwrap();

            let result1 = init_test_localization(&locale, temp_dir.path());
            assert!(result1.is_ok());

            let result2 = init_test_localization(&locale, temp_dir.path());
            assert!(result2.is_err());

            match result2 {
                Err(LocalizationError::Bundle(msg)) => {
                    assert!(msg.contains("already initialized"));
                }
                _ => panic!("Expected Bundle error"),
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_get_message() {
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("fr-FR").unwrap();

            init_test_localization(&locale, temp_dir.path()).unwrap();

            let message = get_message("greeting");
            assert_eq!(message, "Bonjour, le monde!");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_get_message_with_args() {
        use fluent::FluentArgs;
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("en-US").unwrap();

            init_test_localization(&locale, temp_dir.path()).unwrap();

            let mut args = FluentArgs::new();
            args.set("name".to_string(), "Bob".to_string());

            let message = get_message_with_args("welcome", args);
            assert_eq!(message, "Welcome, Bob!");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_get_message_with_args_pluralization() {
        use fluent::FluentArgs;
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("en-US").unwrap();

            init_test_localization(&locale, temp_dir.path()).unwrap();

            let mut args1 = FluentArgs::new();
            args1.set("count", 1);
            let message1 = get_message_with_args("count-items", args1);
            assert_eq!(message1, "You have 1 item");

            let mut args2 = FluentArgs::new();
            args2.set("count", 5);
            let message2 = get_message_with_args("count-items", args2);
            assert_eq!(message2, "You have 5 items");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_thread_local_isolation() {
        use std::thread;

        let temp_dir = create_test_locales_dir();

        let temp_path_main = temp_dir.path().to_path_buf();
        let main_handle = thread::spawn(move || {
            let locale = LanguageIdentifier::from_str("fr-FR").unwrap();
            init_test_localization(&locale, &temp_path_main).unwrap();
            let main_message = get_message("greeting");
            assert_eq!(main_message, "Bonjour, le monde!");
        });
        main_handle.join().unwrap();

        let temp_path = temp_dir.path().to_path_buf();
        let handle = thread::spawn(move || {

            let thread_message = get_message("greeting");
            assert_eq!(thread_message, "greeting");

            let en_locale = LanguageIdentifier::from_str("en-US").unwrap();
            init_test_localization(&en_locale, &temp_path).unwrap();
            let thread_message_after_init = get_message("greeting");
            assert_eq!(thread_message_after_init, "Hello, world!");
        });

        handle.join().unwrap();

        let final_handle = thread::spawn(move || {

            let final_message = get_message("greeting");
            assert_eq!(final_message, "greeting");
        });
        final_handle.join().unwrap();
    }

    #[test]
    fn test_japanese_localization() {
        use fluent::FluentArgs;
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("ja-JP").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let message = get_message("greeting");
            assert_eq!(message, "こんにちは、世界！");

            let mut args = FluentArgs::new();
            args.set("name".to_string(), "田中".to_string());
            let welcome = get_message_with_args("welcome", args);
            assert_eq!(welcome, "ようこそ、田中さん！");

            let mut count_args = FluentArgs::new();
            count_args.set("count".to_string(), "5".to_string());
            let count_message = get_message_with_args("count-items", count_args);
            assert_eq!(count_message, "5個のアイテムがあります");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_arabic_localization() {
        use fluent::FluentArgs;
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("ar-SA").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let message = get_message("greeting");
            assert_eq!(message, "أهلاً بالعالم！");

            let mut args = FluentArgs::new();
            args.set("name", "أحمد".to_string());
            let welcome = get_message_with_args("welcome", args);
            assert_eq!(welcome, "أهلاً وسهلاً، أحمد！");

            let mut args_zero = FluentArgs::new();
            args_zero.set("count", 0);
            let message_zero = get_message_with_args("count-items", args_zero);
            assert_eq!(message_zero, "لديك لا عناصر");

            let mut args_one = FluentArgs::new();
            args_one.set("count", 1);
            let message_one = get_message_with_args("count-items", args_one);
            assert_eq!(message_one, "لديك عنصر واحد");

            let mut args_two = FluentArgs::new();
            args_two.set("count", 2);
            let message_two = get_message_with_args("count-items", args_two);
            assert_eq!(message_two, "لديك عنصران");

            let mut args_few = FluentArgs::new();
            args_few.set("count", 5);
            let message_few = get_message_with_args("count-items", args_few);
            assert_eq!(message_few, "لديك 5 عناصر");

            let mut args_many = FluentArgs::new();
            args_many.set("count", 15);
            let message_many = get_message_with_args("count-items", args_many);
            assert_eq!(message_many, "لديك 15 عنصر");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_arabic_localization_with_macro() {
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("ar-SA").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let message = translate!("greeting");
            assert_eq!(message, "أهلاً بالعالم！");

            let welcome = translate!("welcome", "name" => "أحمد");
            assert_eq!(welcome, "أهلاً وسهلاً، أحمد！");

            let message_zero = translate!("count-items", "count" => 0);
            assert_eq!(message_zero, "لديك لا عناصر");

            let message_one = translate!("count-items", "count" => 1);
            assert_eq!(message_one, "لديك عنصر واحد");

            let message_two = translate!("count-items", "count" => 2);
            assert_eq!(message_two, "لديك عنصران");

            let message_few = translate!("count-items", "count" => 5);
            assert_eq!(message_few, "لديك 5 عناصر");

            let message_many = translate!("count-items", "count" => 15);
            assert_eq!(message_many, "لديك 15 عنصر");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_mixed_script_fallback() {
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("ar-SA").unwrap();

            let result = init_test_localization(&locale, temp_dir.path());
            assert!(result.is_ok());

            let arabic_message = get_message("greeting");
            assert_eq!(arabic_message, "أهلاً بالعالم！");

            let fallback_message = get_message("missing-in-other");
            assert_eq!(fallback_message, "This message only exists in English");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_unicode_directional_isolation_disabled() {
        use fluent::FluentArgs;
        std::thread::spawn(|| {
            let temp_dir = create_test_locales_dir();
            let locale = LanguageIdentifier::from_str("ar-SA").unwrap();

            init_test_localization(&locale, temp_dir.path()).unwrap();

            let mut args = FluentArgs::new();
            args.set("name".to_string(), "John Smith".to_string());
            let message = get_message_with_args("welcome", args);

            assert!(!message.contains("\u{2068}John Smith\u{2069}"));
            assert_eq!(message, "أهلاً وسهلاً، John Smith！");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_parse_resource_error_includes_snippet() {
        let temp_dir = create_test_locales_dir();
        let locale = LanguageIdentifier::from_str("es-ES").unwrap();

        let result = create_test_bundle(&locale, temp_dir.path());
        assert!(result.is_err());

        if let Err(LocalizationError::ParseResource {
            error: _err,
            snippet,
        }) = result
        {

            assert!(
                snippet.contains("This is { $missing"),
                "snippet was `{snippet}` but did not include the invalid text"
            );
        } else {
            panic!("Expected LocalizationError::ParseResource with snippet");
        }
    }

    #[test]
    fn test_localization_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let loc_error = LocalizationError::from(io_error);

        match loc_error {
            LocalizationError::Io { source: _, path } => {
                assert_eq!(path, PathBuf::from("<unknown>"));
            }
            _ => panic!("Expected IO error variant"),
        }
    }

    #[test]
    fn test_localization_error_uerror_impl() {
        let error = LocalizationError::Bundle("some error".to_string());
        assert_eq!(error.code(), 1);
    }

    #[test]
    fn test_get_message_not_initialized() {
        std::thread::spawn(|| {
            let message = get_message("greeting");
            assert_eq!(message, "greeting");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_detect_system_locale_from_lang_env() {

        let locale_with_encoding = "fr-FR.UTF-8";
        let parsed = locale_with_encoding.split('.').next().unwrap();
        let lang_id = LanguageIdentifier::from_str(parsed).unwrap();
        assert_eq!(lang_id.to_string(), "fr-FR");

        let locale_without_encoding = "es-ES";
        let lang_id = LanguageIdentifier::from_str(locale_without_encoding).unwrap();
        assert_eq!(lang_id.to_string(), "es-ES");

        let default_lang_id = LanguageIdentifier::from_str(DEFAULT_LOCALE).unwrap();
        assert_eq!(default_lang_id.to_string(), "en-US");
    }

    #[test]
    fn test_detect_system_locale_no_lang_env() {

        let original_lang = env::var("LANG").ok();

        unsafe {
            env::remove_var("LANG");
        }

        let result = detect_system_locale();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().to_string(), "en-US");

        if let Some(val) = original_lang {
            unsafe {
                env::set_var("LANG", val);
            }
        } else {
            {}
        }
    }

    #[test]
    fn test_setup_localization_success() {
        std::thread::spawn(|| {

            let original_lang = env::var("LANG").ok();
            unsafe {
                env::set_var("LANG", "en-US.UTF-8");
            }

            let result = setup_localization("test");
            assert!(result.is_ok());

            let message = get_message("test-about");

            assert!(!message.is_empty());

            if let Some(val) = original_lang {
                unsafe {
                    env::set_var("LANG", val);
                }
            } else {
                unsafe {
                    env::remove_var("LANG");
                }
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_setup_localization_falls_back_to_english() {
        std::thread::spawn(|| {

            let original_lang = env::var("LANG").ok();
            unsafe {
                env::set_var("LANG", "de-DE.UTF-8");
            }

            let result = setup_localization("test");
            assert!(result.is_ok());

            let message = get_message("test-about");
            assert!(!message.is_empty());

            if let Some(val) = original_lang {
                unsafe {
                    env::set_var("LANG", val);
                }
            } else {
                unsafe {
                    env::remove_var("LANG");
                }
            }
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_setup_localization_fallback_to_embedded() {
        std::thread::spawn(|| {

            unsafe {
                std::env::set_var("LANG", "en-US");
            }

            let result = setup_localization("test");
            if let Err(e) = &result {
                eprintln!("Setup localization failed: {e}");
            }
            assert!(result.is_ok());

            let message = get_message("test-about");
            assert_eq!(message, "Check file types and compare values.");
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_error_display() {
        let io_error = LocalizationError::Io {
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "File not found"),
            path: PathBuf::from("/test/path.ftl"),
        };
        let error_string = format!("{io_error}");
        assert!(error_string.contains("I/O error loading"));
        assert!(error_string.contains("/test/path.ftl"));

        let bundle_error = LocalizationError::Bundle("Bundle creation failed".to_string());
        let bundle_string = format!("{bundle_error}");
        assert!(bundle_string.contains("Bundle error: Bundle creation failed"));
    }

    #[test]
    fn test_clap_localization_fallbacks() {
        std::thread::spawn(|| {

            let error_msg = get_message("common-error");
            assert_eq!(error_msg, "common-error");

            let tip_msg = get_message("common-tip");
            assert_eq!(tip_msg, "common-tip");

            let result = setup_localization("comm");
            if result.is_err() {

                let _ = setup_localization("test");
            }

            let error_after_init = get_message("common-error");

            assert!(!error_after_init.is_empty());

            let tip_after_init = get_message("common-tip");
            assert!(!tip_after_init.is_empty());

            let unknown_arg_key = get_message("clap-error-unexpected-argument");
            assert!(!unknown_arg_key.is_empty());

            let usage_key = get_message("common-usage");
            assert!(!usage_key.is_empty());
        })
        .join()
        .unwrap();
    }
}

#[cfg(all(test, not(debug_assertions)))]
mod fhs_tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolves_fhs_share_locales_layout() {

        let prefix = TempDir::new().unwrap();
        let bin_dir = prefix.path().join("bin");
        let share_dir = prefix.path().join("share").join("locales").join("cut");
        std::fs::create_dir_all(&share_dir).unwrap();
        std::fs::create_dir_all(&bin_dir).unwrap();

        let exe_dir = bin_dir.as_path();

        let result = resolve_locales_dir_from_exe_dir(exe_dir, "cut")
            .expect("should find locales via FHS path");

        assert_eq!(result, share_dir);
    }
}

