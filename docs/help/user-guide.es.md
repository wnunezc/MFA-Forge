# Guia de Usuario de MFA-Forge

## Resumen
MFA-Forge es un gestor MFA con enfoque local para Windows. Su proposito principal es guardar cuentas TOTP en un vault cifrado y exponer un flujo coherente entre la GUI, la CLI humana, la sesion de agente local y el servidor MCP. La aplicacion esta pensada para que los secretos permanezcan locales, las acciones sensibles sean explicitas y la automatizacion no salte silenciosamente los limites de desbloqueo y autorizaciones.

## Primeros pasos
En la primera ejecucion debes crear una contrasena maestra. Esa contrasena es la llave principal del vault: sin ella no puedes agregar cuentas, importar semillas, exportar backups, rotar la contrasena ni generar codigos.

Despues de ingresar la contrasena maestra, MFA-Forge todavia ejecuta la verificacion adicional de Windows usada en esta linea de release. En la practica, la app solo queda utilizable cuando ambos pasos terminan bien.

Una vez desbloqueada, la ventana principal queda dividida en tres zonas de trabajo:

- el arbol de workspaces a la izquierda
- la lista de cuentas en el centro
- las acciones contextuales y dialogos sobre esa distribucion

La idea es que primero selecciones el contexto y luego operes sobre ese alcance sin cambiar de pantalla.

## Workspaces
Los workspaces son el sistema de agrupacion de cuentas. Sirven para separar tokens por proyecto, cliente, entorno o equipo.

Como funcionan:

- un workspace raiz es el contenedor superior
- un subdirectorio es una ruta anidada dentro de un workspace existente
- una cuenta puede vivir dentro de una ruta de workspace o quedar sin asignar

Por que importan:

- el workspace activo filtra la vista de cuentas
- las cuentas nuevas heredan por defecto el workspace seleccionado
- exportar, restaurar y revisar cambios es mas facil cuando las cuentas estan agrupadas de forma consistente

Si tienes cuentas personales o de emergencia, dejarlas sin workspace puede ayudarte a mantenerlas fuera de carpetas especificas de proyecto.

## Alta de cuentas
MFA-Forge soporta cuatro formas principales de cargar una cuenta TOTP:

1. Alta manual
2. Importacion desde URI `otpauth://`
3. Importacion desde imagen QR
4. Importacion desde archivo compatible

El alta manual es la mejor opcion cuando quieres controlar servicio, usuario, workspace, algoritmo, digitos y periodo de forma directa.

La importacion por URI, QR o archivo es mejor cuando otro sistema ya te entrego la semilla en formato TOTP estandar. En esos casos, MFA-Forge parsea el origen, extrae los campos de la cuenta y guarda el secreto cifrado dentro del vault.

Comportamiento importante:

- los secretos permanecen ocultos en la UI
- los dialogos de importacion limpian el texto sensible al cerrarse
- cambiar metadata no obliga a cambiar el secreto
- editar el secreto es opcional; si dejas el campo vacio se conserva el secreto actual ya cifrado

## Tokens e historial
La ventana de token es la vista operativa para leer un codigo. Cuando la abres desde una fila, MFA-Forge lee el TOTP actual del vault desbloqueado y muestra la cuenta regresiva del periodo vigente.

Que debes esperar al actualizar:

- si el mismo periodo TOTP sigue activo, una actualizacion puede devolver exactamente el mismo codigo
- si el periodo cambio, el codigo visible se actualiza de inmediato
- copiar un codigo solo copia el token actual, no el secreto

El historial tiene otro objetivo. No sirve para leer tokens, sino para recuperar estado.

El dialogo de restauracion te permite:

- inspeccionar snapshots restaurables
- recuperar cuentas eliminadas
- restaurar una version previa visible al vault activo

Usa historial cuando una cuenta fue borrada por error, cuando la metadata se modifico mal o cuando necesitas volver a una version anterior sin reconstruir la cuenta manualmente.

## Backup e importacion
La exportacion crea un backup cifrado de MFA-Forge. Su objetivo es preservar el vault completo en un formato que MFA-Forge pueda reimportar despues.

La importacion tiene un efecto fuerte a proposito: tras validar el archivo, reemplaza el contenido del vault activo por el backup cifrado importado. Esto sirve para recuperacion o migracion entre equipos, pero debe tratarse como una restauracion controlada, no como una fusion.

Practica recomendada:

- crear un backup antes de cambios grandes o importaciones masivas
- guardar los backups en una ubicacion protegida
- confirmar que estas importando exactamente el backup esperado antes de aplicarlo

## Agente local y MCP
La sesion de agente local y el servidor MCP existen para automatizacion local, pero no funcionan como canales permanentemente confiables.

Comportamiento base:

- ambos arrancan con denegacion por defecto
- abrir una sesion requiere el flujo nativo de desbloqueo
- la sesion desbloqueada solo vive mientras el proceso siga activo
- las operaciones sensibles requieren autorizaciones explicitas o prompts dedicados

Ejemplos de acciones protegidas:

- generar un token de una cuenta
- aprovisionar o importar cuentas
- leer historial o auditoria sensible
- rotar la contrasena maestra

Eso significa que la automatizacion es posible, pero sigue acotada por aprobacion explicita del usuario y por la vida de la sesion local.

## Resolucion de problemas
Si el desbloqueo falla:

- confirma primero la contrasena maestra
- luego completa la ventana de verificacion de Windows si aparece
- si la app vuelve a la pantalla inicial, repite el flujo y revisa si hay una ventana nativa fuera de la app principal

Si una importacion falla:

- confirma que el origen siga trayendo una carga `otpauth://` valida
- verifica que el secreto Base32 siga completo
- verifica que la imagen QR seleccionada corresponda realmente a la semilla esperada

Si el token no cambia:

- revisa los segundos restantes del periodo TOTP actual
- prueba otra actualizacion cuando el periodo ya haya vencido

Si una automatizacion es denegada:

- revisa si la sesion sigue abierta
- revisa si la autorizacion requerida expiro o ya fue consumida
- abre de nuevo la sesion local y vuelve a aprobar la accion exacta cuando haga falta
