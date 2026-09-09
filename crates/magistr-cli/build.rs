fn main() {
    // Expose the build target triple to source code via env!("TARGET").
    println!(
        "cargo:rustc-env=TARGET={}",
        std::env::var("TARGET").unwrap()
    );
    // The main thread's stack reserve. Windows gives it 1 MiB where Linux
    // and macOS give 8 MiB, and the debug build of clap's derive for `Cli`
    // needs more than 1 MiB before `main` has run anything (`magistr
    // --version` overflowed). Match the Unix reserve for the binary.
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ENV");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bins=/STACK:8388608");
    }
}
