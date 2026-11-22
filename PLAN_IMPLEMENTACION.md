# Plan de implementación del escritorio del comedor

El proyecto parte de una ventana de demostración en `src/main.rs` con Rust y GPUI Kit 0.10.0. El primer entregable funcional será una atención simulada de principio a fin. Un operador inicia sesión, selecciona a un pensionista, confirma un consumo o un abono con tickets físicos y consulta el resultado en Consumos, Caja y Dashboard.

Del proyecto anterior se toma el mapa de módulos. El diseño de las pantallas y su navegación se definen para esta aplicación.

## Arquitectura y patrones

La aplicación será un **monolito modular organizado por funcionalidad**. `main.rs` inicializa GPUI Kit y `app.rs` compone la ventana, conecta servicios y coordina la navegación.

```text
src/
├── main.rs
├── app.rs
├── features/
│   ├── auth/
│   ├── dashboard/
│   ├── pensioners/
│   ├── identification/
│   ├── consumptions/
│   ├── cash/
│   ├── reports/
│   └── settings/
├── state/
├── services/
├── components/
└── theme/
```

- **Entidades reactivas de GPUI.** Cada pantalla conserva sus formularios, filtros y tareas en su propia entidad. `state/` contiene la sesión y el estado observable del lector. La vista raíz entrega esas entidades a quienes las usan. Los controles persistentes, como `InputState`, se crean al inicializar la vista.
- **Flujo unidireccional y observación.** Los componentes reciben valores y devuelven acciones a quien posee el estado. Usar `notify` para comunicar cambios observables y eventos tipados para resultados puntuales, como la identificación de un pensionista. Conservar las suscripciones durante la vida de la vista que las usa.
- **Adaptador y máquina de estados para el lector.** El lector simulado y SecuGen implementan el mismo contrato en `services/biometric/`. El servicio coordina conexión, identificación, enrolamiento y cancelación. Las vistas reciben estados y resultados. El SDK y el procesamiento de las muestras quedan en el servicio, fuera del hilo de interfaz.

## Etapas y criterios de aceptación

### 1. Ventana y sesión de demostración

- [ ] Extraer la composición de la ventana a `src/app.rs`. Crear `features/auth/`, `features/dashboard/` y `state/session.rs` para abrir y cerrar una sesión simulada.
- [ ] Mostrar en Acceso el formulario de inicio de sesión. El registro de cuentas y la recuperación de contraseña quedan para una etapa posterior, por lo que sus enlaces y pantallas se incorporarán entonces.
- [ ] Configurar una cuenta de demostración y mostrar un error ante credenciales inválidas. Proteger Dashboard con el estado de sesión y abrir las rutas de otros módulos cuando sus flujos estén disponibles.

**Aceptación.** La aplicación abre en Acceso. Las credenciales inválidas muestran un error y mantienen al operador en Acceso. La cuenta de demostración abre Dashboard. Cerrar sesión vuelve a Acceso y bloquea el acceso a Dashboard hasta iniciar sesión de nuevo. `cargo check` pasa.

### 2. Pensionistas e identificación simulada

- [ ] Crear `features/pensioners/` con un listado, búsqueda y ficha sobre datos de prueba. Usar el mismo identificador de pensionista en búsqueda, identificación y consumos.
- [ ] Crear `features/identification/`, `state/reader.rs` y el simulador en `services/biometric/`. Simular coincidencia, huella no reconocida, desconexión y cancelación. Coordinar en `app.rs` la entrega del pensionista identificado al flujo de Consumos.

**Aceptación.** La búsqueda y una coincidencia simulada permiten seleccionar al mismo pensionista. Una cancelación o una huella no reconocida deja la atención sin iniciar y permite reintentar.

### Validaciones tempranas en paralelo con la demostración

- [ ] Probar una actualización de Velopack con dos versiones de la ventana de demostración en Windows x64 y Linux x64. Comprobar que el borrador queda invisible, que la app detecta la versión al publicarla y que vuelve a iniciar después de actualizar. El flujo está descrito en [README.md](README.md).
- [ ] Probar SecuGen en Windows con el lector objetivo. Comprobar apertura, captura, cancelación y recuperación tras desconectar y reconectar. Anotar las restricciones observadas del SDK para la etapa 5.
- [ ] Acordar con el backend el contrato de autenticación (login, tokens/refresh, usuario, roles/permisos y errores `401`/`403`), qué devuelve al confirmar una atención, cómo se identifica un reintento mediante una clave de idempotencia y cómo se consulta una confirmación si se pierde la respuesta. Usar ambos acuerdos como entrada para los adaptadores HTTP de la etapa 6.

### 3. Atención automática, Consumos y Caja

- [ ] En `features/consumptions/attendance.rs`, mostrar la persona seleccionada, los consumos y la cantidad de tickets. En `confirm_attendance.rs`, concentrar la validación y el registro sobre una fuente de demostración en memoria.
- [ ] En `features/cash/`, implementar apertura, cierre, arqueo e historial. Caja consulta los movimientos de tickets generados por Consumos. La ficha del pensionista muestra su saldo individual.
- [ ] Deshabilitar la confirmación mientras se procesa. En `confirm_attendance.rs`, tratar los tres casos: consumo sin tickets, abono solo con tickets y consumo con abono. Registrar ambos movimientos como una sola operación cuando corresponda. Impedir que la misma confirmación se registre dos veces.
- [ ] Probar esas reglas sin abrir GPUI. Si una confirmación contiene tickets y la caja está cerrada, rechazar el intento completo antes de actualizar el consumo, el abono o el saldo.

**Aceptación.** Se puede registrar un consumo, un abono con tickets o ambos. La identificación por sí sola no genera movimientos. Un doble clic registra una sola atención. Con caja cerrada se rechaza el abono con tickets y la operación queda sin cambios. El historial y el saldo reflejan la confirmación.

### 4. Dashboard, Reportes y Configuración

- [ ] Conectar Dashboard y Reportes a los mismos registros de demostración que Consumos y Caja. Mostrar consumos, raciones y tickets con filtros por fecha.
- [ ] Crear `features/settings/` y `services/preferences.rs` para guardar el tema, el correo de **Recordarme** (datos no sensibles) y las preferencias de la estación que requiera la aplicación (como la URL base del servidor si es configurable), fuera del directorio que reemplaza Velopack.

**Aceptación.** Usar un solo caso de prueba: abrir caja, registrar un consumo con tickets y consultar el mismo periodo en todos los módulos. Consumos muestra la atención, Caja muestra un abono por la cantidad correcta de tickets, la ficha del pensionista refleja el saldo resultante y Dashboard y Reportes coinciden con esos registros. Las preferencias guardadas se recuperan al reiniciar. Con esta etapa queda completa la demostración funcional.

### 5. SecuGen y enrolamiento

- [ ] Integrar el adaptador real en `services/biometric/` y el flujo de enrolamiento en `features/pensioners/enrollment.rs` a partir de la prueba temprana en Windows. Usar un hilo dedicado si el dispositivo exige afinidad de hilo o mantiene una captura bloqueante prolongada.
- [ ] Aprovechar la prueba previa de respaldo y restauración con SecuSearch para validar el componente de almacenamiento con datos de prueba. Definir la custodia de plantillas y claves antes de guardar huellas reales.

**Aceptación.** En Windows y con el lector objetivo se puede enrolar, identificar, cancelar y recuperar la conexión. La ventana responde durante la captura y las vistas reciben resultados sin muestras biométricas.

### 6. Backend y confirmación persistente

- [ ] Implementar la autenticación real en `services/api/auth.rs` y conectar `features/auth/` y `state/session.rs` fuera del hilo de interfaz. Deshabilitar **Ingresar** con estado de carga mientras responde el servidor, retirar la cuenta de demostración, guardar tokens y permisos reales en sesión, diferenciar errores de credenciales (`401`/`403`) de errores de red y cerrar sesión si el token expira. Si **Recordarme** mantiene sesión entre reinicios, custodiar el token en el llavero seguro del sistema operativo y nunca en texto plano.
- [ ] Completar los contratos de sesión, pensionistas, caja y consultas. Implementar en `services/api/` los adaptadores HTTP y el contrato de confirmación acordado en las validaciones tempranas.
- [ ] Hacer que el backend valide permisos y confirme el registro de negocio. Consultar el resultado de una confirmación incierta y reutilizar su clave de idempotencia si se reintenta.

**Aceptación.** El inicio de sesión valida contra el backend sin congelar la ventana, muestra errores diferenciados de credenciales o red y devuelve a Acceso cuando expira la sesión. El registro persiste tras reiniciar. Una respuesta perdida permite recuperar el resultado o reintentar sin duplicar el movimiento. El servidor rechaza operaciones sin permiso.

### 7. Distribución y actualizaciones en Windows y Linux con Velopack

- [ ] Integrar el SDK de Velopack antes de abrir GPUI. Consultar GitHub Releases fuera del hilo de interfaz y mostrar la versión disponible. El operador decide cuándo descargar, instalar y reiniciar. [Guía de Velopack para Rust](https://docs.velopack.io/getting-started/rust)
- [ ] Configurar GitHub Actions para validar el tag `vX.Y.Z` contra `Cargo.toml` y compilar con `cargo build --release --locked` en runners nativos de Windows y Linux. Empaquetar con `vpk pack` un instalador `.exe` para Windows x64 y una `.AppImage` para Linux x64, con índices y paquetes de actualización por canal. [Empaquetado para Linux](https://docs.velopack.io/packaging/operating-systems/linux) · [Canales de Velopack](https://docs.velopack.io/packaging/channels)
- [ ] Reunir los resultados de ambos jobs en un solo GitHub Release en borrador por tag. Revisar el borrador y publicar manualmente las versiones estables con **Set as latest release**. [Distribución en GitHub con Velopack](https://docs.velopack.io/distributing/self-hosting#github)
- [ ] Usar `DYLAN_SIGNING_KEY` como secreto exclusivo del job de firma. Adjuntar un solo `SHA256SUMS.txt` para ambas plataformas y su firma Ed25519 `SHA256SUMS.txt.sig` después de verificar firma y hashes. Velopack usa su propio índice para actualizar. La firma separada permite verificar los archivos descargables y la firma de código de Windows se configura por separado si se incorpora Authenticode. [Firma de código con Velopack](https://docs.velopack.io/packaging/signing)

**Aceptación.** Probar dos versiones consecutivas en Windows x64 y en una distribución Linux x64 objetivo. El borrador contiene el instalador, la AppImage, los archivos de actualización y la firma verificada. Al publicar, cada plataforma ofrece su propio paquete y la app reinicia conservando las preferencias. Si la versión incorpora SecuGen, comprobar el lector en Windows. En Linux se usa el simulador o se muestra el lector como no disponible hasta validar un adaptador nativo.

## Decisiones pendientes

- **Efectivo.** Antes de agregar un flujo de pago en efectivo, definir si se registra dentro de Atención automática y cómo se confirma. La primera demostración cubre consumos y tickets físicos.
- **Operación sin conexión.** La integración prevista confirma las operaciones en el backend. Si el comedor necesita registrar sin conexión, definir la persistencia, la sincronización y el tratamiento de duplicados antes de implementar ese modo.

## Verificación por etapa

Ejecutar `cargo fmt --check` y `cargo check` después de los cambios en Rust. Desde la etapa 3, ejecutar `cargo test` para verificar cálculo de importes, confirmación duplicada y rechazo de tickets con caja cerrada. Abrir la aplicación con `cargo run` y recorrer los criterios de aceptación de la etapa. La etapa 5 requiere una comprobación adicional con el lector real en Windows.
