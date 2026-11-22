---
name: comedor-release-windows
description: Prepara y verifica la distribución de Windows x64 del escritorio del comedor con Velopack y GitHub Actions. Usa esta skill para instaladores, actualizaciones, firma de artefactos y Releases en borrador.
---

# Distribución para Windows con Velopack

Lee la etapa de distribución de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md). Consulta las guías oficiales de [Rust](https://docs.velopack.io/getting-started/rust), [distribución en GitHub](https://docs.velopack.io/distributing/self-hosting#github) y [firma de código](https://docs.velopack.io/packaging/signing) para la versión que se use.

## Compilar y preparar el borrador

- Ejecuta las verificaciones de CI en Windows sin acceder a secretos de firma. Para generar un Release, parte de un tag `vX.Y.Z` que coincida con la versión de `Cargo.toml` y ejecuta `cargo build --release --locked` para Windows x64.
- Empaqueta con `vpk pack` desde un directorio que contenga el ejecutable y los archivos de distribución previstos. Mantén el identificador de paquete y el canal estables entre versiones. Entrega al job de release el instalador, el índice `releases.<canal>.json` y los paquetes `.nupkg` que el índice referencie. Conserva un canal de Windows separado del de Linux, como `win-x64`. Coordina el borrador compartido con [comedor-release-linux](../comedor-release-linux/SKILL.md). Incluye DLL de SecuGen únicamente cuando se haya confirmado su redistribución. Mantén las preferencias y los datos de la aplicación fuera del directorio que reemplaza la actualización.
- Prepara un borrador para el mismo tag y verifica sus archivos. Con `vpk upload github`, omite `--publish`. Los reintentos pueden completar el borrador del mismo tag. Un Release publicado se conserva. La publicación se hace por revisión manual: en una versión estable, selecciona **Set as latest release** al pulsar **Publish release**.

## Firma de los archivos descargables

- Conserva la firma separada visible en el Release. Genera un único `SHA256SUMS.txt` con los hashes de los archivos finales de Windows y Linux. Firma ese manifiesto con Ed25519 y adjunta `SHA256SUMS.txt.sig`. Verifica la firma con la clave pública y vuelve a comprobar los hashes antes de preparar el borrador.
- Accede a `DYLAN_SIGNING_KEY` solo desde el job de firma de GitHub Actions para tags de publicación. Mantén la semilla privada en el secreto. Publica o distribuye la clave pública por el canal de verificación del proyecto.
- Velopack consume su propio índice y sus paquetes. La firma separada se usa para verificar los archivos descargables. Si se incorpora Authenticode con un certificado o servicio compatible, configura la firma de Windows en el empaquetado y genera los hashes después de firmar los binarios.

## Verificar instalación y actualización

Integra el SDK de Velopack en el inicio de la app antes de abrir GPUI. Ejecuta la consulta y la descarga fuera del hilo de interfaz. Muestra la versión disponible y deja que el usuario decida cuándo descargar, aplicar y reiniciar.

Prueba dos versiones consecutivas en Windows x64. Instala la anterior y comprueba que el borrador no se ofrezca como actualización. Tras la publicación manual, comprueba detección, descarga, instalación, reinicio y conservación de preferencias. Si la versión incluye SecuGen, verifica también el lector en la app actualizada.
