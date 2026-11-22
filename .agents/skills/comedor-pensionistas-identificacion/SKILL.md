---
name: comedor-pensionistas-identificacion
description: Implementa y verifica Pensionistas, Identificación, el estado observable del lector y el simulador biométrico en Rust con GPUI Kit. Usa esta skill al trabajar en la etapa 2, features/pensioners/, features/identification/, state/reader.rs o el contrato común de services/biometric/.
---

# Pensionistas e identificación biométrica

Consulta la etapa 2 de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md), la skill [comedor-gpui](../comedor-gpui/SKILL.md) y [comedor-secugen](../comedor-secugen/SKILL.md) para mantener el contrato compatible con el adaptador real de la etapa 5.

## Contrato único del lector y máquina de estados

- Define en `src/services/biometric/` el contrato común que implementan tanto el simulador (desarrollo y Linux) como el futuro adaptador SecuGen (Windows).
- Expón en `src/state/reader.rs` el estado observable del lector para que las vistas reaccionen sin conocer detalles del hardware ni recibir muestras o plantillas biométricas.

| Estado / evento del lector | Comportamiento esperado |
|---|---|
| Desconectado | Indica lector no disponible y permite reconectar o usar búsqueda manual de pensionistas. |
| Listo | Permite iniciar identificación o enrolamiento de forma exclusiva (una operación activa a la vez). |
| Capturando | Mantiene la ventana responsiva fuera del hilo de GPUI y habilita la cancelación en cualquier momento. |
| Coincidencia | Emite un resultado tipado con el identificador canónico del pensionista. |
| Huella no reconocida | Deja la atención sin iniciar, informa con icono y etiqueta y permite reintentar. |
| Cancelada | Invalida el intento en curso para evitar resultados tardíos y deja listo el siguiente intento. |

## Pensionistas, búsqueda y coordinación

- Implementa `src/features/pensioners/` con listado, búsqueda rápida y ficha sobre datos de prueba (respetando los estilos de [comedor-ui-theme](../comedor-ui-theme/SKILL.md)).
- Usa **un único identificador de pensionista** compartido entre búsqueda, ficha, identificación simulada y el flujo de Consumos.
- En la Etapa 2, coordina en `src/app.rs` la selección del pensionista por búsqueda o por huella simulada. El flujo de esta etapa termina con su identificador canónico seleccionado.
- En la Etapa 3, pasa ese identificador a Consumos para abrir **Atención automática**. Los consumos y abonos se registran cuando el operador confirma la atención.

## Verificación

- Ejecuta `cargo fmt --check`, `cargo check --locked` y `cargo test --locked`.
- Prueba sin abrir GPUI la búsqueda de pensionistas, la máquina de estados del lector y los escenarios del simulador: coincidencia, huella no reconocida, desconexión/reconexión y cancelación sin resultados tardíos.
- En ventana (`cargo run --locked`), comprueba que la búsqueda manual y la coincidencia simulada seleccionan al mismo pensionista. Una cancelación o una huella no reconocida permite reintentar y conserva el estado anterior de la selección.
