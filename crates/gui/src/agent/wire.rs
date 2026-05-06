use std::fmt::Write as _;

use serde::Serialize;

/// Serializa JSON para stdout escapando todo caracter no ASCII como `\uXXXX`.
///
/// Esto mantiene el stream interoperable con clientes Windows que decodifican el
/// pipe con una code page local en vez de tratarlo como UTF-8 puro.
pub fn to_ascii_safe_json<T>(payload: &T) -> Result<String, serde_json::Error>
where
    T: Serialize,
{
    serde_json::to_string(payload).map(|json| escape_non_ascii_json(&json))
}

/// Variante pretty-print para contenido textual embebido dentro del protocolo.
pub fn to_ascii_safe_json_pretty<T>(payload: &T) -> Result<String, serde_json::Error>
where
    T: Serialize,
{
    serde_json::to_string_pretty(payload).map(|json| escape_non_ascii_json(&json))
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
    fn escapes_non_ascii_as_json_unicode_sequences() {
        let json = to_ascii_safe_json(&Sample {
            message: "sesion válida ñ",
        })
        .expect("json should serialize");

        assert!(json.is_ascii());
        assert!(json.contains("\\u00E1"));
        assert!(json.contains("\\u00F1"));

        let parsed: Value = serde_json::from_str(&json).expect("escaped json should parse");
        assert_eq!(parsed["message"], "sesion válida ñ");
    }
}
