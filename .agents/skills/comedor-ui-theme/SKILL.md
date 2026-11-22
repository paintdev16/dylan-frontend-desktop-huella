---
name: comedor-ui-theme
description: Aplica y revisa el tema visual del escritorio del comedor en Rust con GPUI Kit. Usa esta skill al crear o modificar pantallas, tarjetas, botones o estilos compartidos en modo claro u oscuro.
---

# Tema visual del comedor

Usa `src/theme/` como fuente de verdad para colores, tipografía y espaciado. Revisa el tema y `src/components/` antes de crear estilos. Si faltan tokens, define los que requiera la pantalla actual. Las vistas seleccionan variantes semánticas y los componentes compartidos aplican sus estilos.

## Paleta base

- El violeta azulado identifica las acciones principales, la selección y el foco. Como punto de partida, usa violeta `#6438E8` en claro, lavanda `#A78BFA` para acentos en oscuro y azul noche `#11152E` para el fondo oscuro. Ajusta cada combinación de fondo y texto según su contraste.
- Usa superficies neutras para formularios, tablas y contenido de lectura. En Acceso puedes aplicar un degradado violeta a azul al botón **Iniciar sesión** y un brillo suave al fondo.
- Define por separado los colores de fondo, superficie, texto, borde y foco, así como los de éxito, advertencia y error. Acompaña estados críticos con una etiqueta o icono además del color.

## Tarjetas de métricas

| Métrica | Acento |
|---|---|
| Desayunos | Ámbar |
| Almuerzos | Naranja |
| Efectivo ingresado | Verde |
| Tickets recibidos | Azul |
| Saldo a favor | Turquesa |
| Deuda pendiente | Rojo suave |
| Pensionistas registrados, pensionistas activos y raciones | Neutro o lavanda suave |

Mantén el acento de cada métrica en Dashboard, Caja y Reportes. Aplica un tinte suave de fondo y usa un icono o borde más marcado. Prioriza el contraste de la cifra y la etiqueta. Adapta los tonos por modo para conservar el significado y la legibilidad.

## Variantes de botones

| Variante | Uso |
|---|---|
| Principal | Violeta sólido para la acción prioritaria, como **Confirmar atención**. En Acceso se admite el degradado de la paleta base. |
| Secundaria | Superficie neutra con borde sutil para acciones de apoyo, como **Ver detalle** o **Exportar reporte**. |
| Terciaria | Texto o icono sobre fondo transparente para acciones discretas, como **Volver** o **Limpiar filtros**. |
| Destructiva | Rojo y una etiqueta que indique la consecuencia, como **Anular atención**. |
| Contextual | Acento suave de la categoría para acciones ligadas a una tarjeta, como **Ver tickets** en azul. |

Destaca la acción principal dentro de cada pantalla o diálogo. Mantén **Confirmar atención** como acción principal violeta aunque el selector de tickets sea azul. Define las variantes y sus estados de puntero, foco, presión y deshabilitado en el tema o en el componente compartido.

## Verificación

Comprueba en ambos temas habilitados que el texto y los iconos se leen, que el foco de teclado es visible y que los botones deshabilitados se distinguen. Si hay un degradado detrás del texto, revisa el contraste en todo el tramo que ocupa la etiqueta. Cuando cambies un acento compartido, comprueba sus usos en los módulos que muestran la misma métrica.
