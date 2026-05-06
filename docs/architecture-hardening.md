# Architecture Hardening Notes

Este documento fija la frontera arquitectónica y de release después del refactor de RC15.

## Objetivo

- mantener `core` como dominio puro
- mantener `application` como capa compartida de use-cases y sesión
- mantener `storage` como infraestructura del vault
- mantener `platform-windows` como frontera única para Win32/WinRT
- impedir que `gui`, `agent` y `mcp` vuelvan a capturar lógica sensible

## Superficie de binarios aprobada en RC15

Se mantiene esta superficie distribuible:

- `mfa-forge.exe`: CLI humana y puente explícito hacia automatización local
- `mfa-forge-gui.exe`: shell visual desktop
- `mfa-forge-agent.exe`: sesión local por proceso sobre `stdio`
- `mfa-forge-mcp.exe`: servidor MCP local sobre `stdio`

Se elimina de la distribución:

- `mfa-forge-prompt.exe`

Razón:

- el patrón de helper oculto con `CREATE_NO_WINDOW` aumentaba complejidad operativa
- introducía una cadena de procesos más difícil de defender ante AV/EDR
- ahora los prompts nativos viven in-process dentro del host ya visible y controlado

## Regla de fronteras

- `core` no depende de ningún crate del workspace
- `application` no depende de `gui` ni de `platform-windows`
- `platform-windows` es el único crate autorizado a depender de APIs Win32/WinRT
- `gui` y la automatización local consumen flujos compartidos de `application`
- ningún flujo sensible nuevo debe nacer en `gui/src/app.rs`, `agent/stdio.rs` o `agent/mcp.rs`

## Launcher y updater

No se implementa updater en RC15.

La decisión actual es diferirlo hasta tener:

- firma Authenticode consistente
- manifiesto de release firmado
- validación de hash por artefacto
- estrategia de rollback
- criterio claro de reputación AV/SmartScreen

Si se implementa más adelante, debe ser:

- binario separado
- sin acceso al vault ni a secretos
- sin background stealth
- sin auto-actualización opaca ni cadenas de procesos ocultos

## Mitigaciones estructurales para AV

- menos binarios auxiliares
- cero helpers ocultos para prompts
- prompts nativos in-process para unlock, grants y rotación de contraseña
- arranque MCP y agent solo por `stdio`, sin loopback nuevo
- separación explícita de plataforma Windows
- instalación MSI con inventario estable de binarios

## Deuda congelada

Hasta validar esta base no se debe abrir:

- loopback API
- nuevas superficies MCP
- updater real
- nuevas features MFA avanzadas
- mutaciones MCP adicionales fuera del set ya aprobado (`create_account`, `import_otpauth`, `update_account`, `remove_account`, `rotate_master_password`)
- grants o lecturas sensibles fuera del set ya aprobado (`grant_generate_token`, `grant_account_provisioning`, `grant_audit_reporting`, `list_history`, `read_audit_events`, `summarize_audit_events`, `export_metadata`)

## Regla anti-deriva

Todo cambio futuro debe pasar estas preguntas:

1. ¿La lógica vive en `application` si es reutilizable?
2. ¿La dependencia Win32 quedó encapsulada en `platform-windows`?
3. ¿El cambio agrega un binario nuevo? Si sí, requiere decisión explícita.
4. ¿El cambio registra entradas sensibles, `argv`, URIs `otpauth://`, secretos o TOTP? Si sí, debe rechazarse.
5. ¿El cambio puede hacerse in-process en lugar de abrir un helper lateral? Si sí, debe preferirse.

## Validación mínima de una RC

- `cargo fmt --all --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- build release del workspace
- MSI generado con la misma superficie documentada
- verificación manual de unlock GUI
- verificación manual de `mfa-forge agent`
- verificación manual de `mfa-forge mcp`
- confirmación explícita del usuario antes de marcar la RC como válida
