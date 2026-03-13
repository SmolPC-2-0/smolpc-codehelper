use app_lib::launcher::catalog;
use app_lib::launcher::types::LauncherRegistryApp;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(|value| value.as_str()) else {
        return Err(usage());
    };

    match command {
        "register" => run_register(&args[1..]),
        "unregister" => run_unregister(&args[1..]),
        "list" => run_list(),
        _ => Err(usage()),
    }
}

fn run_register(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let app_id = required_flag(&flags, "--app-id")?;
    let exe_path = required_flag(&flags, "--exe-path")?;
    let source = flags
        .get("--source")
        .map(|value| value.to_string())
        .unwrap_or_else(|| "installer".to_string());

    let catalog_doc = catalog::load_catalog_for_process()?;
    let _ = catalog::find_catalog_app(&catalog_doc, app_id)?;

    let path = PathBuf::from(exe_path);
    if !path.is_absolute() {
        return Err("--exe-path must be absolute".to_string());
    }
    if !path.exists() {
        return Err(format!("--exe-path does not exist: {}", path.display()));
    }

    let args_list = optional_string_array(&flags, "--args-json")?.unwrap_or_default();
    let launch_command = optional_string_array(&flags, "--launch-command-json")?;
    let focus_command = optional_string_array(&flags, "--focus-command-json")?;

    let mut entry = LauncherRegistryApp {
        app_id: app_id.to_string(),
        exe_path: path.display().to_string(),
        args: args_list,
        launch_command,
        focus_command,
        installed_at: catalog::now_utc_timestamp(),
        source,
    };
    let registry_path = catalog::resolve_registry_path_for_process()?;
    catalog::upsert_registry_entry_at(&registry_path, &mut entry)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "command": "register",
            "app_id": entry.app_id,
            "exe_path": entry.exe_path,
            "registry_path": registry_path.display().to_string()
        }))
        .map_err(|error| format!("Failed to render register response JSON: {error}"))?
    );
    Ok(())
}

fn run_unregister(args: &[String]) -> Result<(), String> {
    let flags = parse_flags(args)?;
    let app_id = required_flag(&flags, "--app-id")?;

    let catalog_doc = catalog::load_catalog_for_process()?;
    let _ = catalog::find_catalog_app(&catalog_doc, app_id)?;

    let registry_path = catalog::resolve_registry_path_for_process()?;
    let removed = catalog::remove_registry_entry_at(&registry_path, app_id)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "command": "unregister",
            "app_id": app_id,
            "removed": removed,
            "registry_path": registry_path.display().to_string()
        }))
        .map_err(|error| format!("Failed to render unregister response JSON: {error}"))?
    );
    Ok(())
}

fn run_list() -> Result<(), String> {
    let registry_path = catalog::resolve_registry_path_for_process()?;
    let registry = catalog::load_registry_for_process()?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "ok": true,
            "command": "list",
            "registry_path": registry_path.display().to_string(),
            "registry": registry
        }))
        .map_err(|error| format!("Failed to render list response JSON: {error}"))?
    );
    Ok(())
}

fn parse_flags(args: &[String]) -> Result<HashMap<String, String>, String> {
    let mut flags = HashMap::new();
    let mut index = 0usize;
    while index < args.len() {
        let key = args[index].as_str();
        if !key.starts_with("--") {
            return Err(format!(
                "Unexpected argument '{}'. {}",
                args[index],
                usage()
            ));
        }
        index += 1;
        if index >= args.len() {
            return Err(format!("Missing value for flag '{key}'. {}", usage()));
        }
        flags.insert(key.to_string(), args[index].clone());
        index += 1;
    }
    Ok(flags)
}

fn required_flag<'a>(flags: &'a HashMap<String, String>, key: &str) -> Result<&'a str, String> {
    flags
        .get(key)
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Missing required flag '{key}'. {}", usage()))
}

fn optional_string_array(
    flags: &HashMap<String, String>,
    key: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(raw) = flags.get(key) else {
        return Ok(None);
    };
    let parsed = serde_json::from_str::<Vec<String>>(raw)
        .map_err(|error| format!("Flag {key} must be a JSON string array: {error}"))?;
    if parsed.is_empty() {
        return Ok(None);
    }
    Ok(Some(parsed))
}

fn usage() -> String {
    [
        "Usage:",
        "  launcher-register register --app-id <id> --exe-path <abs-path> [--args-json <json>] [--focus-command-json <json>] [--launch-command-json <json>] [--source <text>]",
        "  launcher-register unregister --app-id <id>",
        "  launcher-register list",
    ]
    .join("\n")
}
