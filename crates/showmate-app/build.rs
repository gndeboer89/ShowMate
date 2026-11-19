fn main() {
    // Brontaal van de UI is Nederlands; `lang/<taal>/LC_MESSAGES/showmate-app.po` bevat vertalingen.
    let config = slint_build::CompilerConfiguration::new().with_bundled_translations("lang");
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint-compilatie mislukt");

    embed_windows_icon();
}

/// Zet het programma-pictogram en de versie-informatie in `showmate.exe` (alleen bij Windows-doel).
fn embed_windows_icon() {
    println!("cargo:rerun-if-changed=../../assets/showmate.ico");
    #[cfg(windows)]
    {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
            return;
        }
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/showmate.ico");
        res.set("ProductName", "ShowMate");
        res.set("FileDescription", "ShowMate - cue-software voor shows");
        res.set(
            "LegalCopyright",
            "Copyright (C) 2026 Geert de Boer. GPL-3.0-or-later.",
        );
        if let Err(e) = res.compile() {
            // Zonder icoon werkt het programma gewoon; geen reden om de build te laten falen.
            println!("cargo:warning=icoon niet ingesloten: {e}");
        }
    }
}
