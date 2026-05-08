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

La decisión original en `RC15` fue diferir el updater hasta endurecer packaging y validación. La línea actual ya implementa una versión acotada del updater y su frontera queda fijada así:

- `mfa-forge-launcher.exe` sigue siendo binario separado
- no toca vault ni secretos
- solo descubre releases públicas, valida checksum y delega a `msiexec`
- el trigger puede venir del arranque de la GUI, pero el trabajo sensible sigue fuera de la app principal
- el helper oculto actual existe solo para copiar el launcher fuera del directorio instalado, cerrar la GUI y permitir que la MSI reemplace los binarios en uso
- no existe daemon residente, servicio de fondo ni scheduler persistente

## Aclaracion crítica posterior

La existencia de `mfa-forge-launcher.exe` no equivale a tener updater operativo dentro del producto instalado.

Desde el 2026-05-07 queda fijada esta regla:

- no se puede afirmar soporte de update real si la MSI no instala el launcher cuando ese flujo depende de él
- no se puede afirmar auto-update por apertura de GUI si no existe lógica explícita y verificada de startup update
- esta brecha debía haberse detectado y resuelto desde la implementación inicial del launcher; se trata como fallo crítico de packaging/release, no como detalle documental

En la línea `RC20`:

- la MSI vuelve a incluir `mfa-forge-launcher.exe`
- la GUI solo expone un trigger explícito para delegar el siguiente RC al launcher instalado
- la ruta exacta `RC19 -> RC20` sigue siendo una actualización manual por MSI, porque la RC19 instalada no contenía launcher

En la línea `RC21` y posteriores:

- la lógica de startup update ya existe dentro de la GUI instalada
- el launcher descubre la prerelease RC más nueva publicada, valida checksum y delega a MSI
- desde `RC25 -> RC26` ya existe una validación literal cerrada con apertura de GUI antes del unlock, helper temporal, checksum público verificado y `msiexec /passive` con `exit code 0`
- cada edge exacto instalado sigue requiriendo validación explícita; no basta con asumir que la mecánica general existe

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
