use std::io::Write;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::super::wire;

pub(super) const JSON_RPC_VERSION: &str = "2.0";
pub(super) const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-11-25", "2025-06-18", "2025-03-26"];

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum IncomingMessage {
    Request(JsonRpcRequest),
    Notification(JsonRpcNotification),
}

#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcRequest {
    pub(super) jsonrpc: String,
    pub(super) id: Value,
    pub(super) method: String,
    #[serde(default)]
    pub(super) params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct JsonRpcNotification {
    pub(super) jsonrpc: String,
    pub(super) method: String,
    #[serde(default)]
    pub(super) params: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(super) struct InitializeParams {
    #[serde(rename = "protocolVersion")]
    pub(super) protocol_version: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct ToolCallParams {
    pub(super) name: String,
    #[serde(default)]
    pub(super) arguments: Option<Value>,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonRpcSuccessResponse<T> {
    jsonrpc: &'static str,
    id: Value,
    result: T,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonRpcErrorResponse {
    jsonrpc: &'static str,
    id: Value,
    error: JsonRpcError,
}

#[derive(Debug, Serialize)]
pub(super) struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

#[derive(Debug, Serialize)]
pub(super) struct InitializeResult {
    #[serde(rename = "protocolVersion")]
    pub(super) protocol_version: String,
    pub(super) capabilities: Value,
    #[serde(rename = "serverInfo")]
    pub(super) server_info: ImplementationInfo,
    pub(super) instructions: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ImplementationInfo {
    pub(super) name: &'static str,
    pub(super) version: &'static str,
}

pub(super) enum JsonRpcResponse {
    Success(JsonRpcSuccessResponse<Value>),
    Error(JsonRpcErrorResponse),
}

impl JsonRpcResponse {
    pub(super) fn success<T>(id: Value, result: T) -> Self
    where
        T: Serialize,
    {
        let result = serde_json::to_value(result).unwrap_or_else(|error| {
            json!({
                "content": [{
                    "type": "text",
                    "text": format!("No se pudo serializar la respuesta MCP: {error}")
                }],
                "isError": true
            })
        });

        Self::Success(JsonRpcSuccessResponse {
            jsonrpc: JSON_RPC_VERSION,
            id,
            result,
        })
    }

    pub(super) fn error(id: Value, error: JsonRpcError) -> Self {
        Self::Error(JsonRpcErrorResponse::new(id, error))
    }
}

impl Serialize for JsonRpcResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            JsonRpcResponse::Success(response) => response.serialize(serializer),
            JsonRpcResponse::Error(response) => response.serialize(serializer),
        }
    }
}

impl JsonRpcErrorResponse {
    pub(super) fn new(id: Value, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: JSON_RPC_VERSION,
            id,
            error,
        }
    }
}

impl JsonRpcError {
    pub(super) fn parse_error(message: String) -> Self {
        Self {
            code: -32700,
            message,
            data: None,
        }
    }

    pub(super) fn invalid_request(message: String) -> Self {
        Self {
            code: -32600,
            message,
            data: None,
        }
    }

    pub(super) fn method_not_found(message: String) -> Self {
        Self {
            code: -32601,
            message,
            data: None,
        }
    }

    pub(super) fn invalid_params(message: String, data: Option<Value>) -> Self {
        Self {
            code: -32602,
            message,
            data,
        }
    }
}

pub(super) fn parse_params<T>(params: Option<Value>) -> Result<T, JsonRpcError>
where
    T: for<'de> Deserialize<'de>,
{
    let value = params.unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|error| {
        JsonRpcError::invalid_params(
            format!("Los parámetros enviados no cumplen el esquema esperado: {error}"),
            None,
        )
    })
}

pub(super) fn write_json<T>(writer: &mut impl Write, payload: &T) -> Result<(), String>
where
    T: Serialize,
{
    let json = wire::to_ascii_safe_json(payload)
        .map_err(|error| format!("No se pudo serializar la respuesta JSON: {error}"))?;
    writer
        .write_all(json.as_bytes())
        .map_err(|error| format!("No se pudo escribir la respuesta JSON: {error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("No se pudo escribir la respuesta JSON: {error}"))?;
    writer
        .flush()
        .map_err(|error| format!("No se pudo vaciar stdout: {error}"))
}
