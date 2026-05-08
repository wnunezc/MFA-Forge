# MFA-Forge AI agents - tokens y sesiones

Esta guia documenta como obtener tokens para agentes de IA y como administrar sesiones locales en MFA-Forge usando las dos superficies de automatizacion disponibles:

- `mfa-forge-agent`: sesion local simple sobre JSON por `stdio`
- `mfa-forge-mcp`: servidor MCP local sobre JSON-RPC 2.0 por `stdio`

Todo esto es Windows-only en la linea actual.

## 1. Que binario usar

| Opcion | Cuando usarla | Politica para obtener token | Modelo de sesion |
|---|---|---|---|
| `mfa-forge-agent` | Cuando controlas totalmente el proceso local y quieres el camino mas corto | No requiere grant por cuenta; una vez desbloqueado, `generate_token` funciona directo | La sesion se desbloquea al arrancar el proceso y vive hasta `close_session`, EOF o fin del proceso |
| `mfa-forge-mcp` | Cuando quieres una frontera mas estricta para IA, con grants explicitos y mejor introspeccion de estado | Requiere `grant_generate_token` por cuenta, de un solo uso y con TTL corto | El proceso MCP arranca bloqueado; primero hay que hacer `open_session` |

Puedes lanzar cualquiera de estas superficies de dos formas:

```powershell
mfa-forge agent
mfa-forge mcp
```

o llamando los bins dedicados:

```powershell
mfa-forge-agent.exe
mfa-forge-mcp.exe
```

El comando `mfa-forge agent` hace proxy a `mfa-forge-agent.exe`.
El comando `mfa-forge mcp` hace proxy a `mfa-forge-mcp.exe`.

## 2. Regla practica para tokens

- Si quieres el camino mas corto para que una IA local lea cuentas y saque TOTP, usa `mfa-forge-agent`.
- Si quieres que cada token pida aprobacion humana explicita por cuenta y quede dentro del boundary MCP, usa `mfa-forge-mcp`.
- Ninguna de las dos superficies exporta secretos raw.
- `rotate_master_password` siempre abre un prompt nativo y nunca recibe la nueva password por `stdio` o MCP.

## 3. `mfa-forge-agent` - protocolo local simple por `stdio`

### 3.1 Arranque y eventos iniciales

El protocolo usa JSON newline-delimited. La version declarada es:

```text
mfa-forge-agent/v1
```

Al arrancar, el proceso escribe primero un evento de espera y abre la ventana nativa de unlock:

```json
{"event":"unlock_prompt_opened","status":"waiting_user_action","protocol":"mfa-forge-agent/v1","message":"MFA-Forge abrio una ventana nativa temporal para solicitar la contrasena del vault."}
```

Si el unlock sale bien, responde con:

```json
{"event":"session_ready","status":"access_granted","protocol":"mfa-forge-agent/v1","vault_path":"...","capabilities":["ping","session_info","list_accounts","history","generate_token","add_account","import_otpauth","update_account","remove_account","export_metadata","rotate_master_password","close_session"],"windows_reinforced_unlock":"in_review","message":"La sesion queda abierta mientras este proceso siga vivo o hasta recibir close_session."}
```

Si el unlock falla o el usuario cancela:

```json
{"event":"startup_error","status":"access_denied","protocol":"mfa-forge-agent/v1","error":"..."}
```

### 3.2 Shape de solicitud y respuesta

Cada request es una linea JSON con este shape:

```json
{"id":"req-1","command":"list_accounts"}
```

Las respuestas exitosas:

```json
{"id":"req-1","ok":true,"result":{...}}
```

Las respuestas con error:

```json
{"id":"req-1","ok":false,"error":"..."}
```

### 3.3 Comandos soportados

| Command | Campos | Uso |
|---|---|---|
| `ping` | ninguno | sanity check |
| `session_info` | ninguno | estado de la sesion ya desbloqueada |
| `list_accounts` | ninguno | lista cuentas visibles |
| `history` | ninguno | historial publico |
| `generate_token` | `account_id` | genera TOTP sin grant extra |
| `add_account` | `service`, `user`, `secret`, `totp?` | agrega cuenta |
| `import_otpauth` | `uri` | importa un `otpauth://` |
| `update_account` | `account_id`, `service?`, `user?`, `secret?`, `totp?` | actualiza cuenta |
| `remove_account` | `account_id` | elimina cuenta |
| `export_metadata` | ninguno | exporta metadata publica |
| `rotate_master_password` | ninguno | abre prompt nativo de rotacion |
| `close_session` | ninguno | bloquea y cierra la sesion |

### 3.4 Flujo minimo para obtener un token

1. Arranca `mfa-forge agent` o `mfa-forge-agent.exe`.
2. Espera `session_ready`.
3. Pide `list_accounts`.
4. Toma el `account_id` de la cuenta deseada.
5. Llama `generate_token`.
6. Cuando termines, manda `close_session`.

Ejemplo completo:

```powershell
@'
{"id":"list-1","command":"list_accounts"}
{"id":"token-1","command":"generate_token","account_id":"11111111-2222-3333-4444-555555555555"}
{"id":"close-1","command":"close_session"}
'@ | mfa-forge agent
```

Salida relevante del token:

```json
{
  "id": "token-1",
  "ok": true,
  "result": {
    "account_id": "11111111-2222-3333-4444-555555555555",
    "service": "GitHub",
    "user": "user@example.com",
    "code": "123456",
    "generated_at": 1778191555,
    "expires_at": 1778191585,
    "seconds_remaining": 24
  }
}
```

### 3.5 Como administrar la sesion del agent

- La sesion queda abierta mientras el proceso siga vivo.
- Si `stdin` recibe EOF, la sesion se cierra.
- Si llamas `close_session`, la sesion se bloquea y el proceso pide salir.
- `session_info` sirve para confirmar `vault_path`, `account_count` y que el estado siga en `access_granted`.

## 4. `mfa-forge-mcp` - servidor MCP local por `stdio`

### 4.1 Handshake minimo

El servidor habla JSON-RPC 2.0 y exige `initialize` antes de usar tools.
Las versiones de protocolo aceptadas hoy son:

- `2025-11-25`
- `2025-06-18`
- `2025-03-26`

El `initialize` minimo valido es:

```json
{"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{"protocolVersion":"2025-11-25"}}
```

Despues debes enviar:

```json
{"jsonrpc":"2.0","method":"notifications/initialized"}
```

### 4.2 Tools relevantes para sesiones y tokens

| Tool | Uso |
|---|---|
| `health` | estado basico del servidor y del vault |
| `open_session` | abre la ventana nativa de unlock para este proceso MCP |
| `session_info` | devuelve si la sesion esta `locked` o `access_granted` y expone las politicas activas |
| `list_accounts` | lista cuentas visibles |
| `get_account_metadata` | devuelve metadata publica de una cuenta concreta |
| `grant_generate_token` | pide aprobacion humana explicita para una cuenta |
| `generate_token` | genera el token solo si el grant sigue vigente |
| `close_session` | vuelve a bloquear la sesion MCP |

Tools relacionadas que usan el mismo modelo de grants:

| Tool | Grant requerido |
|---|---|
| `create_account`, `import_otpauth`, `update_account`, `remove_account` | `grant_account_provisioning` |
| `list_history`, `read_audit_events`, `summarize_audit_events` | `grant_audit_reporting` |

### 4.3 Politicas de grants

| Grant | Tool que lo pide | TTL | Limite |
|---|---|---|---|
| token | `grant_generate_token` | 30 segundos | 1 uso y solo para una cuenta |
| provisioning | `grant_account_provisioning` | 600 segundos | hasta 10 cuentas por grant |
| audit reporting | `grant_audit_reporting` | 300 segundos | hasta 10 lecturas por grant |

`session_info` devuelve estas politicas en:

- `generate_token_policy`
- `account_provisioning_policy`
- `audit_reporting_policy`

Cada politica incluye `active_grant` para inspeccionar si hay un grant vigente, expirado o vacio.

### 4.4 Flujo minimo para obtener un token por MCP

1. Arranca `mfa-forge mcp` o `mfa-forge-mcp.exe`.
2. Haz `initialize`.
3. Envia `notifications/initialized`.
4. Llama `open_session` y aprueba el unlock nativo.
5. Llama `list_accounts` o `get_account_metadata`.
6. Llama `grant_generate_token` con el `account_id`.
7. El usuario aprueba el prompt `Approve once`.
8. Llama `generate_token` con el mismo `account_id`.
9. Cuando termines, llama `close_session`.

Ejemplo completo:

```powershell
@'
{"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{"protocolVersion":"2025-11-25"}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":"open-1","method":"tools/call","params":{"name":"open_session","arguments":{}}}
{"jsonrpc":"2.0","id":"list-1","method":"tools/call","params":{"name":"list_accounts","arguments":{}}}
{"jsonrpc":"2.0","id":"grant-1","method":"tools/call","params":{"name":"grant_generate_token","arguments":{"account_id":"11111111-2222-3333-4444-555555555555"}}}
{"jsonrpc":"2.0","id":"token-1","method":"tools/call","params":{"name":"generate_token","arguments":{"account_id":"11111111-2222-3333-4444-555555555555"}}}
{"jsonrpc":"2.0","id":"close-1","method":"tools/call","params":{"name":"close_session","arguments":{}}}
'@ | mfa-forge mcp
```

Respuesta tipica del grant:

```json
{
  "jsonrpc": "2.0",
  "id": "grant-1",
  "result": {
    "content": [{"type":"text","text":"..."}],
    "structuredContent": {
      "status": "granted",
      "message": "Se aprobo un grant explicito de un solo uso para generate_token.",
      "grant": {
        "operation": "generate_token",
        "status": "active",
        "expires_at_epoch_ms": 1778191556045,
        "account_id": "11111111-2222-3333-4444-555555555555",
        "remaining_uses": 1
      }
    },
    "isError": false
  }
}
```

Respuesta tipica del token MCP:

```json
{
  "jsonrpc": "2.0",
  "id": "token-1",
  "result": {
    "content": [{"type":"text","text":"..."}],
    "structuredContent": {
      "token": {
        "account_id": "11111111-2222-3333-4444-555555555555",
        "service": "GitHub",
        "user": "user@example.com",
        "code": "123456",
        "generated_at": 1778191555,
        "expires_at": 1778191585,
        "seconds_remaining": 24
      }
    },
    "isError": false
  }
}
```

### 4.5 Como administrar la sesion MCP

- El proceso MCP arranca bloqueado.
- `open_session` desbloquea la sesion para ese proceso.
- `session_info` te dice si la sesion esta `locked` o `access_granted`.
- `close_session` vuelve a bloquear la sesion, pero no necesariamente mata el proceso.
- Si el proceso termina o `stdin` recibe EOF, MFA-Forge cierra la sesion local.
- `health` es util incluso antes del unlock para revisar `vault_initialized`, `session_open` y si los grants son obligatorios.

## 5. Diferencias importantes entre agent y MCP

| Tema | `mfa-forge-agent` | `mfa-forge-mcp` |
|---|---|---|
| Unlock al arrancar | si | no |
| `generate_token` necesita grant | no | si |
| Inspeccion de politicas activas | basica | completa via `session_info` |
| Handshake adicional | no | si, JSON-RPC + MCP initialize |
| Mejor opcion para otra IA con control fino | aceptable | recomendada |

## 6. Restricciones y garantias

- No hay export de secretos raw.
- No hay export de TOTP seeds.
- `export_metadata` solo devuelve metadata publica.
- `rotate_master_password` usa prompt nativo y no mueve la nueva password por `stdio`.
- Todo el boundary de automatizacion es local al host Windows.

## 7. Recomendacion operativa

- Usa `mfa-forge-agent` si la IA es totalmente local, confiable y solo necesitas una sesion corta para listar cuentas o sacar TOTP rapido.
- Usa `mfa-forge-mcp` si quieres flujo mas estricto, grants por operacion y mejor trazabilidad de sesion para otra IA.
