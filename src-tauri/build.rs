fn main() {
    // Windows/MSVC 测试二进制需要 Common Controls v6 manifest：
    // tauri/test 的 mock runtime 链入 TaskDialogIndirect（comctl32 v6 独有），
    // 产品 bin 由 tauri_build 注入 manifest，测试 exe 必须在此单独注入，
    // 否则测试进程加载即失败（STATUS_ENTRYPOINT_NOT_FOUND）。
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("test-common-controls.manifest");
        println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
            manifest.display()
        );
        println!("cargo:rerun-if-changed=test-common-controls.manifest");
    }
    tauri_build::build()
}
