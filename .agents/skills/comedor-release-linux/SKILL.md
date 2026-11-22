---
name: comedor-release-linux
description: Genera y verifica la distribución Linux x64 del escritorio del comedor como AppImage con Velopack. Coordina los paquetes de Linux y Windows en un GitHub Release en borrador.
---

# Distribución para Linux con Velopack

Lee la etapa de distribución de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md), la skill [comedor-release-windows](../comedor-release-windows/SKILL.md) y la guía oficial de [Velopack para Linux](https://docs.velopack.io/packaging/operating-systems/linux).

## Compilar y empaquetar

- Compila en un runner Linux x64 con `cargo build --release --locked`. Valida que el tag coincida con `Cargo.toml` y declara qué distribuciones se probaron. Verifica en ellas las dependencias de GPUI y que abra la ventana desde la AppImage.
- Usa `vpk pack` con el ejecutable Linux, su icono PNG y un canal separado de Windows, como `linux-x64`. Conserva el identificador del paquete y el canal entre versiones. Entrega la `.AppImage`, el índice `releases.linux-x64.json` y los paquetes que necesita el índice.
- Mantén las preferencias fuera de la AppImage. Mientras no exista un adaptador Linux validado para el lector, utiliza el simulador o muestra el lector como no disponible. Empaqueta las DLL de Windows únicamente en su distribución correspondiente.

## Preparar el Release compartido

- Los jobs de Windows y Linux compilan y empaquetan por separado. Un job de release espera a ambos y reúne los archivos en un solo borrador para el tag. Revisa que los índices, los paquetes y el instalador o AppImage tengan nombres distintos y correspondan a su plataforma.
- Genera un único `SHA256SUMS.txt` para los archivos finales de ambas plataformas. Fírmalo con Ed25519 usando el secreto `DYLAN_SIGNING_KEY` en el job de firma. Verifica la firma y los hashes antes de adjuntar `SHA256SUMS.txt.sig` y los paquetes al borrador.
- Deja la publicación a revisión manual. Al publicar una versión estable, selecciona **Set as latest release**.

## Verificar Linux

En una distribución Linux x64 objetivo, da permiso de ejecución a la AppImage con `chmod +x` y ejecútala desde una ubicación donde el usuario pueda escribir. Prueba dos versiones consecutivas. Comprueba que el borrador no se ofrezca, que el Release publicado active la detección y que la actualización reinicie la app conservando las preferencias. Verifica que el actualizador seleccione el canal Linux y que Windows seleccione su propio canal.
