use std::fs;

fn main() {
    let version_file = "../VERSION";
    println!("cargo:rerun-if-changed={version_file}");
    let expected = fs::read_to_string(version_file)
        .expect("无法读取根目录 VERSION 文件")
        .trim()
        .to_string();
    let cargo_version = env!("CARGO_PKG_VERSION");
    if expected != cargo_version {
        panic!(
            "VERSION ({expected}) 与 src-tauri/Cargo.toml ({cargo_version}) 不一致。\
             请运行 `npm run version:sync`，只修改根目录 VERSION 文件。"
        );
    }
    tauri_build::build()
}
