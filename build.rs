use std::env;
use std::path::PathBuf;

fn main() {
    let libraries = [
        ("LIBUBUS_PATH", "libubus"),
        ("LIBUBOX_PATH", "libubox"),
        ("LIBBLOBMSG_JSON_PATH", "libblobmsg_json"),
    ];

    for (env_var, lib_name) in libraries.iter() {
        if let Ok(path) = env::var(env_var) {
            println!("cargo:rustc-link-search=native={}", path);
            println!("cargo:rustc-link-lib={}", lib_name);
        } else {
            println!("Looking for {} using pkg-config...", lib_name);
            pkg_config::Config::new()
                .probe(lib_name)
                .unwrap_or_else(|_| {
                    panic!(
                        "Failed to find {}. Set the {} environment variable or install it with pkg-config.",
                        lib_name, env_var
                    );
                });
        }

        // Rebuild if environment variables change
        println!("cargo:rerun-if-env-changed={}", env_var);
    }

    println!("cargo:rerun-if-changed=build.rs");

    let include_path = std::env::var("LIBUBUS_INCLUDE_PATH").ok();

    let include_path = include_path.or_else(|| {
        pkg_config::probe_library("libubus").ok().and_then(|lib| {
            lib.include_paths
                .get(0)
                .map(|path| path.to_str().expect("Invalid UTF-8 in include path").to_string())
        })
    });

    let include_path = include_path.expect(
        "Could not find libubus. Either set LIBUBUS_INCLUDE_PATH to the include directory, \
        or ensure libubus is installed and pkg-config is available.",
    );

    // Generate bindings for libubus.h
    let bindings = bindgen::Builder::default()
        .header(format!("{}/libubus.h", include_path))
        .derive_copy(false)
        .derive_debug(false)
        .derive_default(true)
        .layout_tests(false)
        .generate_inline_functions(true)
        .clang_arg("-v")
        .clang_arg(format!("-I/usr/include"))
        .clang_arg(format!("-I/usr/local/include"))
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // Finish the builder and generate the bindings.
        .generate()
        .expect("Unable to generate bindings");

    bindings
        .write_to_file(PathBuf::from("src/libubus/libubus_bindgen.rs"))
        .expect("Couldn't write bindings!");
}
