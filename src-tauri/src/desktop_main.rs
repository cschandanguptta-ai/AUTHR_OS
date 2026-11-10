//! `authr-desktop` — the Tauri v2 desktop shell binary (Phase 2).
//!
//! Built only with `cargo build --features desktop` (enforced via
//! `required-features` in Cargo.toml). All application logic lives in
//! `authr_core::tauri_shell`.

#[cfg(feature = "desktop")]
fn main() {
    authr_core::tauri_shell::run();
}

#[cfg(not(feature = "desktop"))]
fn main() {
    // Unreachable in practice: the binary is gated by `required-features`,
    // but keep a helpful message for `cargo run --bin authr-desktop` without
    // the feature flag.
    eprintln!(
        "authr-desktop requires the `desktop` feature: \
         cargo build --release --features desktop"
    );
    std::process::exit(2);
}
