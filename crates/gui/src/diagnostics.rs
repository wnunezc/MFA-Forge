use std::{
    any::Any,
    env,
    fs::{self, OpenOptions},
    io::Write,
    panic::{self, AssertUnwindSafe},
    path::PathBuf,
    sync::Once,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use directories::ProjectDirs;
use serde_json::{Value, json};

static PANIC_HOOK_ONCE: Once = Once::new();

pub fn trace_enabled() -> bool {
    env::var_os("MFA_FORGE_TRACE_LOG").is_some() || env::var_os("MFA_FORGE_UI_TRACE").is_some()
}

pub fn trace_log_path() -> Option<PathBuf> {
    if let Some(path) = env::var_os("MFA_FORGE_TRACE_LOG_PATH") {
        return Some(PathBuf::from(path));
    }

    ProjectDirs::from("dev", "OpsZone", "MFA-Forge").map(|dirs| {
        dirs.data_local_dir()
            .join("logs")
            .join("runtime-trace.jsonl")
    })
}

pub fn log_event(component: &str, event: &str, details: Value) {
    if !trace_enabled() {
        return;
    }

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default();
    let entry = json!({
        "ts_epoch_ms": timestamp,
        "pid": std::process::id(),
        "thread": format!("{:?}", std::thread::current().id()),
        "component": component,
        "event": event,
        "details": details,
    });

    if let Err(error) = append_json_line(&entry) {
        eprintln!(
            "[mfa-forge-diagnostics] write_failed component={component} event={event} error={error}"
        );
    }
}

pub fn install_panic_hook(component: &'static str) {
    PANIC_HOOK_ONCE.call_once(|| {
        let component = component.to_owned();
        let default_hook = panic::take_hook();

        panic::set_hook(Box::new(move |panic_info| {
            log_event(
                &component,
                "panic",
                json!({
                    "message": panic_info_message(panic_info),
                    "location": panic_info
                        .location()
                        .map(|location| format!("{}:{}", location.file(), location.line())),
                }),
            );
            default_hook(panic_info);
        }));
    });
}

pub fn guard_result<T, E, F>(component: &str, operation: &str, action: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, E>,
    F: std::panic::UnwindSafe,
    E: std::fmt::Display,
{
    let started_at = Instant::now();
    log_event(
        component,
        &format!("{operation}.start"),
        operation_start_details(),
    );

    match panic::catch_unwind(AssertUnwindSafe(action)) {
        Ok(Ok(value)) => {
            log_event(
                component,
                &format!("{operation}.ok"),
                json!({
                    "elapsed_ms": started_at.elapsed().as_millis(),
                }),
            );
            Ok(value)
        }
        Ok(Err(error)) => {
            let error = error.to_string();
            log_event(
                component,
                &format!("{operation}.error"),
                json!({
                    "elapsed_ms": started_at.elapsed().as_millis(),
                    "error": error,
                }),
            );
            Err(error)
        }
        Err(payload) => {
            let panic = panic_payload_message(payload);
            log_event(
                component,
                &format!("{operation}.panic"),
                json!({
                    "elapsed_ms": started_at.elapsed().as_millis(),
                    "panic": panic,
                }),
            );
            Err(format!(
                "El proceso '{operation}' terminó por panic: {panic}"
            ))
        }
    }
}

fn operation_start_details() -> Value {
    json!({
        "argv_redacted": true,
    })
}

pub fn panic_payload_message(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(message) => *message,
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => (*message).to_owned(),
            Err(_) => "panic without string payload".to_owned(),
        },
    }
}

fn append_json_line(entry: &Value) -> Result<(), String> {
    let path = trace_log_path().ok_or_else(|| {
        "No se pudo resolver la ruta del runtime trace para MFA-Forge.".to_owned()
    })?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "No se pudo crear el directorio del runtime trace '{}': {error}",
                parent.display()
            )
        })?;
    }

    let payload = serde_json::to_string(entry)
        .map_err(|error| format!("No se pudo serializar la traza runtime: {error}"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| {
            format!(
                "No se pudo abrir el runtime trace '{}': {error}",
                path.display()
            )
        })?;

    writeln!(file, "{payload}")
        .map_err(|error| format!("No se pudo escribir el runtime trace: {error}"))
}

fn panic_info_message(panic_info: &panic::PanicHookInfo<'_>) -> String {
    if let Some(message) = panic_info.payload().downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = panic_info.payload().downcast_ref::<String>() {
        message.clone()
    } else {
        "panic without string payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::operation_start_details;

    #[test]
    fn operation_start_details_redacts_argv() {
        let details = operation_start_details();

        assert_eq!(
            details
                .get("argv_redacted")
                .and_then(|value| value.as_bool()),
            Some(true)
        );
        assert!(details.get("args").is_none());
    }
}
