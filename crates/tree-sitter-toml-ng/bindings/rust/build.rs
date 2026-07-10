fn main() {
    let src_dir = std::path::Path::new("src");

    let mut c_config = cc::Build::new();
    c_config
        .std("c11")
        .include(src_dir)
        .flag_if_supported("-Wno-unused-parameter");

    #[cfg(target_env = "msvc")]
    c_config.flag("-utf-8");

    if std::env::var("TARGET").as_deref() == Ok("wasm32-unknown-unknown") {
        let wasm_headers = std::env::var("DEP_TREE_SITTER_LANGUAGE_WASM_HEADERS")
            .expect("tree-sitter-language must provide portable WebAssembly headers");
        let wasm_src = std::env::var("DEP_TREE_SITTER_LANGUAGE_WASM_SRC")
            .map(std::path::PathBuf::from)
            .expect("tree-sitter-language must provide portable WebAssembly sources");

        c_config.include(wasm_headers);
        c_config.files([
            wasm_src.join("stdio.c"),
            wasm_src.join("stdlib.c"),
            wasm_src.join("string.c"),
        ]);
    }

    let parser_path = src_dir.join("parser.c");
    c_config.file(&parser_path);
    println!("cargo:rerun-if-changed={}", parser_path.to_str().unwrap());

    let scanner_path = src_dir.join("scanner.c");
    c_config.file(&scanner_path);
    println!("cargo:rerun-if-changed={}", scanner_path.to_str().unwrap());

    c_config.compile("tree-sitter-toml");
}
