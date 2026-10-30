fn main() {
    // The CurseForge API key lives in an untracked file so it never ends up in source control.
    println!("cargo:rerun-if-changed=cf_key.txt");
    let key = std::fs::read_to_string("cf_key.txt").unwrap_or_default();
    println!("cargo:rustc-env=JC_CF_KEY={}", key.trim());
    tauri_build::build()
}
