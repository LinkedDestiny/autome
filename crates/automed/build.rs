mod build_support;

fn main() {
    let dir = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_support.rs");
    for path in build_support::repository_watch_paths(&dir) {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let id = build_support::compute(&dir).unwrap_or_else(|| {
        println!("cargo:warning=构建标识：拿不到 git 提交，标为未知来源");
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        format!(
            "unknown-{}-{}",
            std::env::var("CARGO_PKG_VERSION").unwrap(),
            build_support::utc_timestamp(seconds)
        )
    });
    println!("cargo:rustc-env=AUTOME_BUILD_ID={id}");
}
