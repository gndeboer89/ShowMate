fn main() {
    // Brontaal van de UI is Nederlands; `lang/<taal>/LC_MESSAGES/showmate-app.po` bevat vertalingen.
    let config = slint_build::CompilerConfiguration::new().with_bundled_translations("lang");
    slint_build::compile_with_config("ui/main.slint", config).expect("Slint-compilatie mislukt");
}
