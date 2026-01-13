use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let sdk_home = env::var("ECOS_SDK_HOME").expect("ECOS_SDK_HOME not set");
    let sdk_path = PathBuf::from(&sdk_home);

    let include_dirs = scan_sdk_directories(&sdk_path);
    generate_bindings(&sdk_path, &include_dirs);

    println!("cargo:rerun-if-env-changed=ECOS_SDK_HOME");
    println!("cargo:rerun-if-changed=include/wrapper.h");
}

fn scan_sdk_directories(sdk_path: &Path) -> Vec<PathBuf> {
    let mut include_dirs = Vec::new();

    include_dirs.push(PathBuf::from("./include"));

    let board_path = sdk_path.join("board/StarrySkyC1");
    if board_path.exists() {
        include_dirs.push(board_path.clone());
        scan_directory(&board_path, &mut include_dirs);
    }

    for dir_name in &["components", "devices"] {
        let dir_path = sdk_path.join(dir_name);
        if dir_path.exists() {
            include_dirs.push(dir_path.clone());
            scan_directory(&dir_path, &mut include_dirs);
        }
    }

    include_dirs.sort();
    include_dirs.dedup();

    include_dirs
}

fn scan_directory(dir: &Path, include_dirs: &mut Vec<PathBuf>) {
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current_dir) = stack.pop() {
        let entries = fs::read_dir(&current_dir).expect("Failed to read directory");
        let mut has_h = false;

        for entry in entries {
            let path = entry.expect("Failed to get directory entry").path();

            if path.is_file() {
                if let Some(ext) = path.extension() {
                    let ext_str = ext.to_str().expect("Invalid extension");
                    if ext_str.eq_ignore_ascii_case("h") {
                        has_h = true;
                    }
                }
            } else if path.is_dir() {
                stack.push(path);
            }
        }

        if has_h {
            include_dirs.push(current_dir);
        }
    }
}

fn generate_bindings(_sdk_path: &Path, include_dirs: &[PathBuf]) {
    let mut clang_args = vec!["-mabi=ilp32".to_string(), "-march=rv32imac".to_string()];

    for dir in include_dirs {
        if dir.exists() {
            clang_args.push(format!("-I{}", dir.display()));
        }
    }

    let bindings = bindgen::Builder::default()
        .header("include/wrapper.h")
        .clang_args(clang_args)
        .use_core()
        .ctypes_prefix("cty")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("bindgen failed");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    bindings
        .write_to_file(PathBuf::from(&out_dir).join("bindings.rs"))
        .expect("write bindings failed");
}
