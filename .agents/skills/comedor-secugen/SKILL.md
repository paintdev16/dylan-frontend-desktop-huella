---
name: comedor-secugen
description: Integra y verifica la biometría SecuGen del escritorio del comedor en Windows. Usa esta skill para captura con FDx SDK Pro, identificación con SecuSearch Pro, enrolamiento y respaldo cifrado.
---

# Integración biométrica con SecuGen

Consulta las validaciones tempranas y la etapa 5 de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md). Distingue las pruebas del motor de búsqueda de las pruebas con el lector físico.

## SDK y evidencia existente

| Dependencia | Función en el proyecto | Referencia local |
|---|---|---|
| FDx SDK Pro para Windows 4.3.1 | Abrir el lector, capturar una huella y obtener su plantilla. | Paquete `FDx_SDK_Pro_Windows_v4.3.1_J1.21`, subcarpeta del SDK para Windows. |
| SecuSearch Pro para Windows | Enrolar plantillas en el motor, identificar una coincidencia 1:N y cargar o guardar su base. | Paquete extraído en `Nueva carpeta`, con la DLL `bin/secusearchapi.dll` y los encabezados de API en `inc/`. |

El ejemplo `secugen-rust`, dentro de la primera carpeta, carga la DLL de la segunda con `libloading` y usa `test_data/ansi378_2views.bin` como entrada de prueba. La ejecución compartida guardó y restauró un respaldo con AES-256-GCM de 200 registros de prueba y luego identificó una plantilla. Esa comprobación corresponde a SecuSearch usando un archivo de prueba. El ZIP está rotulado como 3.5.1 y el motor reportó 3.5.0. Registra la versión que devuelve la biblioteca cargada al diagnosticar diferencias.

## Integración con Rust y GPUI

- Ubica el adaptador real en `src/services/biometric/` y conéctalo mediante el mismo contrato que utiliza el simulador. Configura las ubicaciones de los SDK por instalación. Comprueba que la arquitectura del ejecutable coincida con la de cada DLL y usa los encabezados del paquete correspondiente como referencia para las firmas FFI.
- Mantén la vida del dispositivo y del motor dentro del adaptador. Ejecuta la captura y el procesamiento fuera del hilo de GPUI. Si el SDK requiere afinidad de hilo o mantiene una captura bloqueante prolongada, usa un hilo dedicado al lector. Coordina identificación y enrolamiento para que accedan al lector de forma exclusiva.
- Publica estados y resultados tipados a la interfaz. Conserva las muestras y plantillas dentro de la integración biométrica. Al cancelar, invalida el intento para evitar que un resultado tardío se aplique a la siguiente operación.
- Usa datos de prueba para verificar el respaldo. Define dónde se guardan las plantillas y cómo se custodia la clave antes de incorporar huellas reales. Mantén claves, muestras y respaldos fuera del repositorio y de los logs.

## Verificación en Windows

1. **Motor SecuSearch.** Verifica la carga de la DLL y su versión. Con datos de prueba, comprueba respaldo cifrado, restauración, cantidad recuperada e identificación posterior.
2. **Lector físico con FDx.** Comprueba apertura, captura, cancelación y recuperación después de desconectar y reconectar. Anota el modelo, la versión del SDK y el controlador que funcionaron.
3. **Flujo integrado en GPUI.** Enrola desde la ficha de un pensionista y verifica la identificación con el lector. Comprueba que la ventana responde durante la captura y que cancelar o desconectar el equipo permite iniciar un intento posterior en condiciones válidas.

Informa por separado qué comprobaciones ejecutaste con datos de prueba y cuáles completaste con el lector físico.
