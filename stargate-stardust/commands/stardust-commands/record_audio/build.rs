fn main() {
    #[cfg(all(target_os = "macos", feature = "transcription"))]
    {
        println!("cargo:rustc-link-search=native=/usr/local/lib");
        println!("cargo:rustc-link-lib=dylib=vosk");
    }
}
