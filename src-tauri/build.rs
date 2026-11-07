fn main() {
    // Tauri codegen only when the desktop shell is being built.
    #[cfg(feature = "gui")]
    {
        tauri_build::build();
    }
}
