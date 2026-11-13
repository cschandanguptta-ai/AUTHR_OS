// AUTHR_OS — CLI entry (headless / tests / scripting).
// Single-line JSON envelope on stdout, exit 0/1 (docs/CLI_CONTRACT.md §1–2).

use clap::Parser;
use serde_json::{json, Value};
use std::io::Write;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "authr",
    version,
    about = "AUTHR_OS core runtime — 74-command IPC surface (CLI transport)"
)]
struct Cli {
    /// Contract command name (see docs/CLI_CONTRACT.md §4)
    command: Option<String>,

    /// `.authr` bundle directory
    #[arg(long, value_name = "BUNDLE_DIR")]
    bundle: Option<PathBuf>,

    /// JSON object with the command arguments
    #[arg(long, value_name = "JSON", conflicts_with = "contract")]
    args: Option<String>,

    /// Read the args JSON object from stdin instead of argv. Additive
    /// transport option (contract §1): large payloads — e.g. a bulk Lexical
    /// AST mutation — can exceed the OS per-argument limit (≈128 KB on
    /// Linux); the dev bridge switches to this automatically.
    #[arg(long, conflicts_with_all = ["args", "contract"])]
    args_stdin: bool,

    /// Print the frozen command surface and exit
    #[arg(long)]
    contract: bool,
}

fn emit(value: Value) {
    let line = serde_json::to_string(&value).expect("envelope serialization");
    println!("{line}");
    let _ = std::io::stdout().flush();
}

fn main() {
    let cli = Cli::parse();

    // Desktop shell: a bare launch — no command, no --contract, no --bundle,
    // no --args — of a gui-enabled build opens the cockpit instead of
    // printing the CLI usage envelope. release.yml builds the Windows
    // installer with `--features gui,os-keychain`; CLI-only builds (default
    // features, E2E runners) are unchanged.
    #[cfg(feature = "gui")]
    {
        if cli.command.is_none()
            && !cli.contract
            && cli.bundle.is_none()
            && cli.args.is_none()
            && !cli.args_stdin
        {
            authr::shell::run();
            return; // shell::run returns when the main window closes
        }
    }

    if cli.contract {
        emit(json!({
            "ok": true,
            "data": {
                "contract_version": authr::CONTRACT_VERSION,
                "commands": authr::COMMANDS,
                "count": authr::COMMANDS.len(),
            }
        }));
        std::process::exit(0);
    }

    let command = match cli.command.as_deref() {
        Some(c) => c.to_string(),
        None => {
            emit(json!({
                "ok": false,
                "error": {"code": "BUNDLE_ERROR", "message": "usage: authr <command> [--bundle BUNDLE_DIR] [--args JSON] (try --contract)",
                          "category": "bundle"}
            }));
            std::process::exit(1);
        }
    };

    let raw_args: Option<String> = if cli.args_stdin {
        use std::io::Read;
        let mut raw = String::new();
        match std::io::stdin().read_to_string(&mut raw) {
            Ok(_) => Some(raw),
            Err(e) => {
                emit(json!({
                    "ok": false,
                    "error": {"code": "STORAGE_ERROR", "message": format!("--args-stdin read error: {e}"), "category": "storage"}
                }));
                std::process::exit(1);
            }
        }
    } else {
        cli.args.clone()
    };

    let args: Value = match &raw_args {
        Some(raw) => match serde_json::from_str(raw) {
            Ok(v @ Value::Object(_)) => v,
            Ok(_) => {
                emit(json!({
                    "ok": false,
                    "error": {"code": "STORAGE_ERROR", "message": "--args must be a JSON object", "category": "storage"}
                }));
                std::process::exit(1);
            }
            Err(e) => {
                emit(json!({
                    "ok": false,
                    "error": {"code": "STORAGE_ERROR", "message": format!("--args parse error: {e}"), "category": "storage"}
                }));
                std::process::exit(1);
            }
        },
        None => json!({}),
    };

    match authr::ipc::dispatch(cli.bundle.as_deref(), &command, &args) {
        Ok(data) => {
            emit(json!({ "ok": true, "data": data }));
            std::process::exit(0);
        }
        Err(err) => {
            emit(json!({ "ok": false, "error": err.to_json() }));
            std::process::exit(1);
        }
    }
}
