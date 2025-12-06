use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

pub fn main() {
    const ENV_FEATURE_PREFIX: &str = "CARGO_FEATURE_";
    const FEATURE_PREFIX: &str = "feat_";
    const OVERRIDE_PREFIX: &str = "sg_";

    println!("cargo:rerun-if-changed=build.rs");

    if let Ok(profile) = env::var("PROFILE") {
        println!("cargo:rustc-cfg=build={profile:?}");
    }

    let out_dir = env::var("OUT_DIR").unwrap();

    let mut crates = Vec::new();
    for (key, val) in env::vars() {
        if val == "1" && key.starts_with(ENV_FEATURE_PREFIX) {
            let krate = key[ENV_FEATURE_PREFIX.len()..].to_lowercase();
            #[allow(clippy::match_same_arms)]
            match krate.as_ref() {
                "default" | "macos" | "unix"| "zip" | "clap_complete"
                | "clap_mangen" | "fluent_syntax" => continue,
                "nightly" | "test_unimplemented" | "expensive_tests" | "test_risky_names" => {
                    continue;
                }
                "uudoc" => continue,
                "test" => continue,
                s if s.starts_with(FEATURE_PREFIX) => continue,
                _ => {}
            }
            crates.push(krate);
        }
    }
    crates.sort();

    let mut mf = File::create(Path::new(&out_dir).join("uutils_map.rs")).unwrap();

    mf.write_all(
        "type UtilityMap<T> = phf::OrderedMap<&'static str, (fn(T) -> i32, fn() -> Command)>;\n\
         \n\
         #[allow(clippy::too_many_lines)]
         #[allow(clippy::unreadable_literal)]
         fn util_map<T: sgcore::Args>() -> UtilityMap<T> {\n"
            .as_bytes(),
    )
    .unwrap();

    let util_names: Vec<String> = crates.iter()
        .map(|name| name.replace('_', "-"))
        .collect();

    let mut phf_map = phf_codegen::OrderedMap::<&str>::new();
    for (idx, krate) in crates.iter().enumerate() {
        let map_value = format!("({krate}::sgmain, {krate}::sg_app)");
        match krate.as_ref() {
            "sg_test" => {
                phf_map.entry("test", map_value.clone());
                phf_map.entry("[", map_value.clone());
            }
            "false" | "true" => {
                phf_map.entry(&util_names[idx], format!("(r#{krate}::sgmain, r#{krate}::sg_app)"));
            }
            "hashsum" => {
                phf_map.entry(&util_names[idx], format!("({krate}::sgmain, {krate}::sg_app_custom)"));

                let map_value = format!("({krate}::sgmain, {krate}::sg_app_common)");
                phf_map.entry("md5sum", map_value.clone());
                phf_map.entry("sha1sum", map_value.clone());
                phf_map.entry("sha224sum", map_value.clone());
                phf_map.entry("sha256sum", map_value.clone());
                phf_map.entry("sha384sum", map_value.clone());
                phf_map.entry("sha512sum", map_value.clone());
                phf_map.entry("b2sum", map_value.clone());
            }
            "get_linenumber" => {
                phf_map.entry(&util_names[idx], map_value.clone());
                phf_map.entry("nl", map_value.clone());
            }
            _ => {
                phf_map.entry(&util_names[idx], map_value.clone());
            }
        }
    }
    write!(mf, "{}", phf_map.build()).unwrap();
    mf.write_all(b"\n}\n").unwrap();

    mf.flush().unwrap();
}
