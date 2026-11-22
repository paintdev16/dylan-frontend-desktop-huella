---
name: comedor-gpui
description: Implementa pantallas, navegación y estado reactivo del escritorio del comedor en Rust con GPUI Kit. Usa esta skill al trabajar en Acceso, Dashboard, el flujo de sesión o la estructura de nuevas vistas.
---

# Implementar flujos del comedor con GPUI

Lee la etapa aplicable de [PLAN_IMPLEMENTACION.md](../../../PLAN_IMPLEMENTACION.md) antes de modificar el código. Implementa el flujo solicitado de extremo a extremo desde su entrada habitual. Para decisiones de paleta, tarjetas y botones, consulta la skill [comedor-ui-theme](../comedor-ui-theme/SKILL.md).

## Ubicar el código

- Mantén en `src/main.rs` la inicialización de GPUI Kit y la apertura de la ventana. Usa `src/app.rs` para componer las vistas, conectar los servicios y coordinar la navegación.
- Coloca los formularios, filtros y tareas en la vista que los usa dentro de `src/features/`. Extrae a `src/state/` el estado que varias vistas necesitan observar. Pasa esas entidades a las vistas desde su punto de composición.
- Si el flujo requiere hardware, red o disco, conecta la integración desde `src/services/`. Crea el contrato que necesita ese flujo cuando haya una integración concreta. Mantén las llamadas bloqueantes fuera del hilo de la interfaz.

## Estado y eventos de GPUI

- Crea las entidades persistentes, como `InputState`, al inicializar la vista. En `render()` compón los elementos a partir del estado actual. Conserva la identidad de los controles que necesitan mantener el foco o su contenido.
- Deja que la entidad dueña del estado procese la acción. Actualiza mediante `Entity::update` y llama a `cx.notify()` cuando cambie el estado que se muestra. Usa `observe` para cambios de estado compartido y `subscribe` para eventos tipados entre entidades cuando el flujo lo requiera. Conserva cada `Subscription` mientras la vista la necesite.
- Comprueba las firmas de GPUI Kit en la versión que declara el proyecto. Reutiliza los controles y patrones que ya funcionan en el repositorio.

## Etapa 1. Acceso y Dashboard

- Crea o completa `src/features/auth/`, `src/features/dashboard/` y `src/state/session.rs`. El estado de sesión empieza sin autenticar y determina si se puede abrir Dashboard.
- Implementa el formulario de Acceso con la cuenta de demostración prevista en el plan. Si las credenciales son inválidas, muestra el error en Acceso. Si son válidas, abre Dashboard. Al cerrar sesión, vuelve a Acceso y comprueba que Dashboard queda inaccesible hasta autenticarse otra vez.
- Habilita rutas adicionales cuando sus flujos estén disponibles. Los enlaces y pantallas de registro de cuentas y recuperación de contraseña pertenecen a una etapa posterior.

## Configuración y preferencias (Etapa 4)

- Implementa `src/features/settings/` y `src/services/preferences.rs` para persistir preferencias locales no sensibles: tema (`Sistema / Claro / Oscuro`), correo de **Recordarme** y parámetros de estación (como la URL base del servidor si es configurable).
- Guarda el archivo de preferencias en el directorio de datos de usuario del sistema operativo (fuera del directorio de instalación que sobrescribe Velopack al actualizar) y recárgalo al iniciar sin bloquear la interfaz.
- Nunca guardes contraseñas, tokens de sesión, claves de firma ni datos biométricos en el archivo de preferencias.

## Verificación

Ejecuta `cargo fmt --check` y `cargo check` desde la raíz. Cuando puedas abrir la app con `cargo run`, verifica inicio en Acceso, credenciales inválidas, ingreso válido y cierre de sesión. Indica en el resultado cualquier comprobación de ventana que haya quedado pendiente.
