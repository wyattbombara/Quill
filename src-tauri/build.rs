fn main() {
    tauri_build::build();
    // Native dialogs import Common Controls v6. Integration test executables
    // need the same application manifest that Tauri links into the desktop binary.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out = std::env::var("OUT_DIR").expect("Cargo supplies OUT_DIR");
        let resource = if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
            "resource.lib"
        } else {
            "libresource.a"
        };
        println!("cargo:rustc-link-arg-tests={out}/{resource}");
    }
}
