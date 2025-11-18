use std::env;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();

    cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_language(cbindgen::Language::C)
        .with_documentation(true)
        .with_include_guard("GRAPHLITE_H")
        .with_no_includes()
        .with_pragma_once(true)
        .generate()
        .expect("Unable to generate C bindings")
        .write_to_file("graphlite.h");

    println!("cargo:rerun-if-changed=src/lib.rs");
}
