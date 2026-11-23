---
name: comedor-gpui
description: Implementa pantallas, navegación y estado reactivo del escritorio del comedor en Rust con GPUI Kit. Usa esta skill al trabajar en Acceso, Dashboard, el flujo de sesión, el sidebar, el buscador en la barra de título o la estructura de nuevas vistas.
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

## Sidebar del área autenticada

- Mantén la ruta activa y el estado expandido o colapsado en `src/app.rs`. Confía en las rutas existentes para derivar las entradas del sidebar. Colapsarlo conserva la ruta y la instancia de la vista activa.
- Ubica el sidebar reutilizable en `src/components/sidebar.rs`. Recibe las rutas disponibles, la ruta activa y los callbacks de selección y colapso. Usa etiquetas e iconos al expandirlo e iconos con nombres accesibles y ayudas emergentes al colapsarlo. Señala la sección activa en ambos estados.
- Agrupa las rutas en **Inicio** (Dashboard), **Operación** (Pensionistas, Identificación y Consumos), **Administración** (Caja y Reportes) y **Sistema** (Configuración). Muestra únicamente las rutas implementadas y permitidas para la sesión. Comprueba sesión y permisos también al navegar mediante acciones o atajos. Al cerrar sesión, restablece la ruta inicial y vuelve a Acceso.
- Aplica los tokens de superficie, selección y foco del tema compartido. Coloca el sidebar al lado del contenido autenticado y conserva el fondo compartido en las vistas. Mantén el buscador global en la barra de título.

## Buscador de pensionistas en la barra de título

- Crea una sola entidad para el buscador y mantenla en `src/app.rs`. Presenta **Buscar pensionista** en `src/components/title_bar.rs` cuando la sesión autenticada tenga permiso de búsqueda. Cambiar de vista o colapsar el sidebar conserva la misma entidad.
- Registra `Ctrl+K` en la ventana principal para enfocar el campo desde cualquier vista autorizada. Permite recorrer coincidencias con las flechas y seleccionar con `Enter`. Comprueba el permiso también al ejecutar el atajo. Cuando Atención automática esté abierta, conserva el foco del modal.
- Al cerrar sesión, limpia la consulta y la selección pendiente. Verifica que el buscador conserva el foco y funciona al cambiar de vista y colapsar el sidebar, y que el atajo queda inactivo en Acceso.

## Configuración y preferencias (Etapa 4)

- Implementa `src/features/settings/` y `src/services/preferences.rs` para persistir preferencias locales no sensibles: tema (`Sistema / Claro / Oscuro`), correo de **Recordarme** y parámetros de estación (como la URL base del servidor si es configurable).
- Guarda el archivo de preferencias en el directorio de datos de usuario del sistema operativo (fuera del directorio de instalación que sobrescribe Velopack al actualizar) y recárgalo al iniciar sin bloquear la interfaz.
- Nunca guardes contraseñas, tokens de sesión, claves de firma ni datos biométricos en el archivo de preferencias.

## Verificación

Ejecuta `cargo fmt --check` y `cargo check` desde la raíz. Cuando puedas abrir la app con `cargo run`, verifica inicio en Acceso, credenciales inválidas, ingreso válido y cierre de sesión. Al cambiar el sidebar, verifica que colapsar y expandir conserva la sección activa, que una ruta sin permiso queda inaccesible y que cerrar sesión oculta la navegación. Comprueba el foco de teclado y la selección en claro y oscuro. Indica en el resultado cualquier comprobación de ventana que haya quedado pendiente.
