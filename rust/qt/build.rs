use std::{env, path::PathBuf, process::Command};

fn output(program: &str, args: &[&str]) -> String {
    let result = Command::new(program)
        .args(args)
        .output()
        .expect("failed to run build tool");
    assert!(
        result.status.success(),
        "{program} failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).expect("build tool emitted non-UTF-8")
}
fn main() {
    println!("cargo:rerun-if-changed=src/frontend.cpp");
    println!("cargo:rerun-if-changed=src/generator_wheel.h");
    for key in [
        "CXX",
        "AR",
        "OPT_LEVEL",
        "PKG_CONFIG",
        "PKG_CONFIG_PATH",
        "PKG_CONFIG_LIBDIR",
        "PKG_CONFIG_SYSROOT_DIR",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let pkg = env::var("PKG_CONFIG").unwrap_or_else(|_| "pkg-config".into());
    let flags = output(&pkg, &["--cflags", "Qt6Widgets"]);
    let libs = output(&pkg, &["--libs", "Qt6Widgets"]);
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let object = out.join("frontend.o");
    let archive = out.join("libswatch_qt.a");
    let mut compiler = Command::new(env::var_os("CXX").unwrap_or_else(|| "c++".into()));
    let opt = env::var("OPT_LEVEL").unwrap_or_else(|_| "0".into());
    compiler
        .arg(format!("-O{}", if opt == "z" { "s" } else { &opt }))
        .args(["-std=c++17", "-fPIC", "-c", "src/frontend.cpp", "-o"])
        .arg(&object)
        .args(flags.split_whitespace());
    assert!(
        compiler
            .status()
            .expect("failed to run C++ compiler")
            .success(),
        "Qt frontend compilation failed"
    );
    assert!(
        Command::new(env::var_os("AR").unwrap_or_else(|| "ar".into()))
            .arg("crs")
            .arg(&archive)
            .arg(&object)
            .status()
            .expect("failed to run ar")
            .success(),
        "Qt frontend archive failed"
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=swatch_qt");
    for flag in libs.split_whitespace() {
        if let Some(path) = flag.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(lib) = flag.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        } else {
            println!("cargo:rustc-link-arg={flag}");
        }
    }
    println!("cargo:rustc-link-lib=stdc++");
}
