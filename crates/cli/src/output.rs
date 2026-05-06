use anyhow::Result;
use serde::Serialize;

use mfa_forge_core::{AccountHistoryEntryPublic, AccountPublic};

use crate::csv_io::ExternalImportPreviewRow;

use crate::args::OutputFormat;

pub fn print_payload<T>(format: OutputFormat, text: String, payload: &T) -> Result<()>
where
    T: Serialize,
{
    match format {
        OutputFormat::Text => {
            println!("{text}");
        }
        OutputFormat::Json => {
            println!("{}", to_ascii_safe_json_pretty(payload)?);
        }
    }

    Ok(())
}

pub fn render_accounts(accounts: &[AccountPublic]) -> String {
    if accounts.is_empty() {
        return "No MFA accounts stored.".to_owned();
    }

    let mut lines = Vec::with_capacity(accounts.len() + 1);
    lines.push(
        "ID | Service | User | Kind | Algorithm | Digits | Period | Project | Labels | Source"
            .to_owned(),
    );
    lines.extend(accounts.iter().map(|account| {
        format!(
            "{} | {} | {} | {} | {} | {} | {}s | {} | {} | {}",
            account.id,
            account.service,
            account.user,
            account.kind,
            account.totp.algorithm,
            account.totp.digits,
            account.totp.period_seconds,
            account.metadata.project_path.as_deref().unwrap_or("-"),
            if account.metadata.labels.is_empty() {
                "-".to_owned()
            } else {
                account.metadata.labels_csv()
            },
            account.metadata.source.as_deref().unwrap_or("-")
        )
    }));

    lines.join("\n")
}

pub fn render_history(entries: &[AccountHistoryEntryPublic]) -> String {
    if entries.is_empty() {
        return "No hay historial granular de cuentas disponible.".to_owned();
    }

    let mut lines = Vec::with_capacity(entries.len() + 1);
    lines.push("Entry ID | Evento | Cuenta | Proyecto | Capturado".to_owned());
    lines.extend(entries.iter().map(|entry| {
        format!(
            "{} | {} | {} | {} | {}",
            entry.entry_id,
            entry.event.as_str(),
            entry.account.display_name(),
            entry
                .account
                .metadata
                .project_path
                .as_deref()
                .unwrap_or("-"),
            entry.captured_at
        )
    }));

    lines.join("\n")
}

pub fn render_external_import_preview(rows: &[ExternalImportPreviewRow]) -> String {
    if rows.is_empty() {
        return "El origen no contiene cuentas TOTP importables.".to_owned();
    }

    let mut lines = Vec::with_capacity(rows.len() + 1);
    lines.push("Fila | Servicio | Usuario | Proyecto | Labels | Origen".to_owned());
    lines.extend(rows.iter().map(|row| {
        format!(
            "{} | {} | {} | {} | {} | {}",
            row.row_number,
            row.service,
            row.user,
            row.project_path.as_deref().unwrap_or("-"),
            if row.labels.is_empty() {
                "-".to_owned()
            } else {
                row.labels.join(", ")
            },
            row.source
        )
    }));

    lines.join("\n")
}

fn to_ascii_safe_json_pretty<T>(payload: &T) -> Result<String>
where
    T: Serialize,
{
    Ok(escape_non_ascii_json(&serde_json::to_string_pretty(
        payload,
    )?))
}

fn escape_non_ascii_json(input: &str) -> String {
    if input.is_ascii() {
        return input.to_owned();
    }

    let mut output = String::with_capacity(input.len());

    for ch in input.chars() {
        if ch.is_ascii() {
            output.push(ch);
            continue;
        }

        let mut units = [0_u16; 2];
        for &unit in ch.encode_utf16(&mut units).iter() {
            use std::fmt::Write as _;
            let _ = write!(&mut output, "\\u{unit:04X}");
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use serde::Serialize;
    use serde_json::Value;

    use super::*;

    #[derive(Serialize)]
    struct Sample<'a> {
        message: &'a str,
    }

    #[test]
    fn json_output_escapes_non_ascii() {
        let json = to_ascii_safe_json_pretty(&Sample {
            message: "válida ñ",
        })
        .expect("json should serialize");

        assert!(json.is_ascii());
        let parsed: Value = serde_json::from_str(&json).expect("json should parse");
        assert_eq!(parsed["message"], "válida ñ");
    }
}
