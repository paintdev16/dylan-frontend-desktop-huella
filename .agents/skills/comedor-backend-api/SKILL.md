---
name: comedor-backend-api
description: Implementa y verifica la integración HTTP con el backend del comedor en Rust y GPUI Kit. Usa esta skill al trabajar en services/api/, login real con tokens, manejo de errores de red o confirmación idempotente de atenciones y caja.
---

# Integración HTTP con el backend

Consulta las validaciones tempranas y la etapa 6 de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md). Para las reglas de negocio de consumos, tickets y caja, consulta [comedor-atencion-caja](../comedor-atencion-caja/SKILL.md).

## Estructura y contratos

- Ubica los adaptadores HTTP en `src/services/api/` (por ejemplo `auth.rs` y los clientes de pensionistas, consumos, caja y reportes) compartiendo un cliente base con la URL configurada y cabecera `Authorization: Bearer` cuando exista sesión activa.
- Ejecuta todas las solicitudes HTTP fuera del hilo de interfaz de GPUI (`background_executor` + `cx.spawn`) y aplica el resultado sobre las entidades del hilo principal mediante `update` y `cx.notify()`.
- Mantén DTOs y reglas de mapeo desacoplados de las vistas para poder probar serialización, errores y reintentos con `cargo test` sin abrir GPUI.

## Autenticación real y ciclo de sesión

- Reemplaza la cuenta de demostración en `src/features/auth/` y `src/state/session.rs` por el adaptador `src/services/api/auth.rs` acordado con el backend.
- Mientras el servidor responde, deshabilita el botón **Ingresar** con estado de carga e ignora envíos repetidos (clic o `Enter`).
- Guarda en `SessionState` el usuario, rol, permisos y tokens devueltos por el servidor. Por defecto, **Recordarme** guarda solo el correo en las preferencias. Si se decide mantener la sesión entre reinicios, custodia el token en el llavero del sistema operativo (Credential Manager o Secret Service). Reserva `preferences.rs` para el correo y otras preferencias sin credenciales.

| Respuesta o fallo | Comportamiento esperado |
|---|---|
| `200 / 201` en login | Guarda sesión y permisos, limpia la contraseña del formulario y abre Dashboard. |
| `401 / 403` en login | Muestra error de credenciales o cuenta sin permiso y mantiene al operador en Acceso. |
| `401` en operación autenticada | Intenta renovar el token una vez si el contrato lo admite. Si se renueva, actualiza la sesión y reintenta la solicitud según el contrato. Si la renovación falla o el backend no la admite, cierra la sesión local y vuelve a Acceso con un aviso de expiración. |
| `403` en operación autenticada | Rechaza la acción sin cerrar sesión e informa falta de permisos. |
| Timeout, red caída o `5xx` | Muestra error de conexión diferenciado y permite reintentar sin perder el formulario. |

## Confirmación persistente e idempotencia

- Genera una clave de idempotencia estable por intento de confirmación y reutilízala si se reintenta la misma operación tras un fallo de red o respuesta perdida.
- Ante una respuesta incierta, consulta el estado de la confirmación en el backend según el contrato acordado. Si una confirmación se reintenta después de renovar el token, conserva su clave de idempotencia.
- Deja en el servidor la validación final de permisos, estado de caja y consistencia del registro de negocio.

## Verificación

- Ejecuta `cargo fmt --check`, `cargo check --locked` y `cargo test --locked`.
- Cubre con pruebas unitarias (con respuestas simuladas, sin abrir GPUI): login válido, rechazo `401`/`403`, fallo de red, renovación exitosa y fallida, cierre de sesión cuando no hay renovación, reutilización de clave de idempotencia y recuperación tras respuesta perdida.
- En ventana con el backend activo, comprueba que la app no se congela durante la petición, que los registros persisten tras reiniciar y que el servidor rechaza operaciones sin permiso.
