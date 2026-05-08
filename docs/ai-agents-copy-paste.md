# Copy/paste para otra IA

Copia y pega el siguiente bloque tal cual en otra IA si quieres que entienda como operar MFA-Forge para sesiones y tokens:

```text
Estas integrando MFA-Forge en Windows. Hay dos superficies locales para agentes de IA:

1. mfa-forge-agent
   - Es una sesion local simple sobre JSON newline-delimited por stdio.
   - Arranca ya con prompt nativo de unlock.
   - Cuando el usuario desbloquea el vault, generate_token funciona directo y NO requiere grant por cuenta.
   - La sesion vive mientras el proceso siga vivo, hasta EOF o hasta close_session.
   - Requests:
     {"id":"req-1","command":"list_accounts"}
     {"id":"req-2","command":"generate_token","account_id":"UUID"}
     {"id":"req-3","command":"close_session"}
   - Responses:
     {"id":"req-1","ok":true,"result":{...}}
     {"id":"req-1","ok":false,"error":"..."}
   - Comandos soportados:
     ping
     session_info
     list_accounts
     history
     generate_token
     add_account
     import_otpauth
     update_account
     remove_account
     export_metadata
     rotate_master_password
     close_session
   - Ejemplo rapido:
     @'
     {"id":"list-1","command":"list_accounts"}
     {"id":"token-1","command":"generate_token","account_id":"11111111-2222-3333-4444-555555555555"}
     {"id":"close-1","command":"close_session"}
     '@ | mfa-forge agent

2. mfa-forge-mcp
   - Es un servidor MCP local sobre JSON-RPC 2.0 por stdio.
   - Arranca bloqueado.
   - Requiere handshake MCP:
     {"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{"protocolVersion":"2025-11-25"}}
     {"jsonrpc":"2.0","method":"notifications/initialized"}
   - Para desbloquear la sesion hay que llamar:
     {"jsonrpc":"2.0","id":"open-1","method":"tools/call","params":{"name":"open_session","arguments":{}}}
   - Para inspeccionar estado:
     {"jsonrpc":"2.0","id":"info-1","method":"tools/call","params":{"name":"session_info","arguments":{}}}
   - Para sacar un token SIEMPRE haz esta secuencia:
     a. list_accounts
     b. grant_generate_token con account_id
     c. esperar aprobacion humana en el prompt nativo
     d. generate_token con el mismo account_id
   - Ejemplo completo:
     @'
     {"jsonrpc":"2.0","id":"init-1","method":"initialize","params":{"protocolVersion":"2025-11-25"}}
     {"jsonrpc":"2.0","method":"notifications/initialized"}
     {"jsonrpc":"2.0","id":"open-1","method":"tools/call","params":{"name":"open_session","arguments":{}}}
     {"jsonrpc":"2.0","id":"list-1","method":"tools/call","params":{"name":"list_accounts","arguments":{}}}
     {"jsonrpc":"2.0","id":"grant-1","method":"tools/call","params":{"name":"grant_generate_token","arguments":{"account_id":"11111111-2222-3333-4444-555555555555"}}}
     {"jsonrpc":"2.0","id":"token-1","method":"tools/call","params":{"name":"generate_token","arguments":{"account_id":"11111111-2222-3333-4444-555555555555"}}}
     {"jsonrpc":"2.0","id":"close-1","method":"tools/call","params":{"name":"close_session","arguments":{}}}
     '@ | mfa-forge mcp
   - Tools relevantes:
     health
     open_session
     session_info
     list_accounts
     get_account_metadata
     grant_generate_token
     generate_token
     grant_account_provisioning
     create_account
     import_otpauth
     update_account
     remove_account
     grant_audit_reporting
     list_history
     read_audit_events
     summarize_audit_events
     export_metadata
     rotate_master_password
     close_session

Politicas de grant en MCP:
- grant_generate_token: 30 segundos, un solo uso, una sola cuenta
- grant_account_provisioning: 600 segundos, hasta 10 cuentas
- grant_audit_reporting: 300 segundos, hasta 10 lecturas

Diferencia clave:
- mfa-forge-agent es el camino corto y no pide grant extra por token.
- mfa-forge-mcp es el camino recomendado si quieres control fino para otra IA, porque obliga a open_session y a grant_generate_token antes de generate_token.

Garantias:
- No existe export de secretos raw.
- export_metadata solo devuelve metadata publica.
- rotate_master_password siempre usa prompt nativo y nunca envia la nueva password por stdio/MCP.
- Todo esto es local al host Windows.
```
