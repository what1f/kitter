fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");

    #[cfg(target_os = "windows")]
    winresource::WindowsResource::new()
        .set_icon("assets/app-icon.ico")
        .set("ProductName", "Kitter")
        .set("FileDescription", "Kitter")
        .set("LegalCopyright", "Copyright Kitter contributors")
        .compile()
        .expect("failed to embed Windows application resources");
}
