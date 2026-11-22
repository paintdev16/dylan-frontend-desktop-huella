---
name: comedor-atencion-caja
description: Implementa y verifica la confirmación de Atención automática, los consumos, los abonos con tickets físicos y su reflejo en Caja, saldos y reportes del escritorio del comedor.
---

# Atención automática y Caja

Consulta las etapas 3 y 4 de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md) antes de cambiar el flujo. Si trabajas con el backend, consulta también el contrato de confirmación acordado para la etapa 6.

## Responsabilidades

- **Identificación** o la búsqueda entrega el identificador del pensionista. Seleccionar una persona abre el flujo de atención. El registro empieza cuando el operador confirma.
- **Consumos** presenta Atención automática en `src/features/consumptions/attendance.rs`. Centraliza la validación y la escritura en `confirm_attendance.rs`, de forma que las reglas se puedan probar sin abrir GPUI.
- **Caja** gestiona apertura, cierre, arqueo e historial. La ficha del pensionista muestra el saldo individual. Consumos, Caja, Pensionistas, Dashboard y Reportes consultan la misma fuente de registros confirmados.

## Reglas de confirmación

| Solicitud | Resultado |
|---|---|
| Consumo sin tickets | Registra el consumo y actualiza el saldo. Puede confirmarse con caja cerrada. |
| Abono solo con tickets | Requiere caja abierta. Registra el abono y actualiza el saldo. |
| Consumo con abono en tickets | Requiere caja abierta. Confirma el consumo y el abono en una sola operación. |
| Solicitud con tickets y caja cerrada | Rechaza toda la operación. El consumo, el abono y el saldo conservan sus valores previos. |
| Solicitud sin consumos ni tickets | Rechaza la confirmación por falta de movimientos. |

Valida el estado de caja al confirmar. Deshabilita el botón mientras se procesa y protege el registro contra duplicados también en la operación de escritura. Con el backend integrado, conserva la clave de idempotencia de la misma operación si se pierde la respuesta y consulta o reintenta según el contrato acordado.

Registra cantidad de tickets e importe del abono por separado. La implementación de pagos en efectivo depende de la decisión indicada en el plan.

## Verificación

- Ejecuta `cargo test` para cubrir los tres casos válidos, el rechazo completo con caja cerrada y una confirmación repetida. Comprueba los cambios concretos en movimientos y saldo.
- Recorre un caso integral con caja abierta. Confirma un consumo con tickets: Consumos debe mostrar la atención, Caja debe mostrar un único abono por los tickets y la ficha debe mostrar el saldo resultante. Donde Dashboard y Reportes ya estén implementados, consulta el mismo periodo y comprueba que sus cifras coincidan con los registros.
