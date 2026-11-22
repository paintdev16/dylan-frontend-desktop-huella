# Dylan Comedor

Aplicación de escritorio en Rust y GPUI Kit. El proyecto incluye una ventana de demostración para probar el instalador y el actualizador mientras se desarrollan los módulos del [plan de implementación](PLAN_IMPLEMENTACION.md).

## Desarrollo

```sh
cargo run --locked
```

La versión generada por GitHub Actions incluye el repositorio de origen como fuente de actualizaciones. Usa un repositorio público para que la app instalada pueda consultar los Releases sin un token de GitHub. El código de desarrollo puede compilarse sin esa configuración.

## Preparar un Release

1. Sube el proyecto a tu repositorio de GitHub. Configura el secreto de Actions `DYLAN_SIGNING_KEY` con la semilla Ed25519 de 32 bytes en 64 caracteres hexadecimales. Conserva la misma clave entre versiones y distribuye su clave pública por un canal confiable.
2. De forma opcional, ejecuta **Build and prepare desktop release** con **Run workflow** para descargar y probar los paquetes de Windows x64 y Linux x64 desde los artefactos de Actions.
3. Sube un tag `vX.Y.Z` que coincida con la versión de `Cargo.toml`. Windows y Linux compilan y empaquetan por separado. Un solo job reúne y firma los archivos, verifica sus hashes y prepara el Release como borrador.
4. Revisa el instalador de Windows, la AppImage de Linux, los archivos de actualización y la firma. Publica la versión estable con **Set as latest release**.

Para probar una actualización, instala una versión anterior y prepara la siguiente. El borrador queda invisible para la app. Después de publicarlo, abre la versión anterior o pulsa **F5** para buscar. La app también comprueba cada 30 minutos mientras está abierta. Al encontrar la nueva versión, elige **Descargar** y luego **Instalar actualización** en la barra de título. En Linux ejecuta la AppImage con permiso de ejecución desde una ubicación donde puedas escribir.

## Verificar los archivos descargados

Desde el directorio de descarga, con OpenSSL 3 y las herramientas de GNU instaladas, verifica los archivos con la clave pública que guardaste por separado.

```sh
openssl pkeyutl -verify -pubin -inkey DYLAN_SIGNING_PUBLIC_KEY.pem -rawin \
  -in SHA256SUMS.txt -sigfile SHA256SUMS.txt.sig
sha256sum --check --strict SHA256SUMS.txt
```

`SHA256SUMS.txt.sig` es la firma separada de los archivos publicados. Velopack utiliza los índices `releases.win-x64.json` y `releases.linux-x64.json` y sus paquetes. La firma de código Authenticode se configura aparte si se incorpora un certificado o servicio compatible.
