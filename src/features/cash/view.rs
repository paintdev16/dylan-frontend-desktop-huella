use gpui_kit::{
    base::FocusTrapElement as _,
    component::{
        Icon, IconName, Sizable as _, StyledExt as _,
        alert::Alert,
        h_flex,
        input::{Input, InputState},
        label::Label,
        searchable_list::SearchableListItem,
        select::Select,
        tab::{Tab, TabBar},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::{
        button::AppButton,
        loading,
        metric_card::MetricCard,
        status_badge::{StatusBadge, StatusTone},
    },
    features::pensioners::model::format_soles,
    state::store::{
        CashMovement, CashMovementKind, CashSession, ClosedCashSession, DataLoad, DiningLedger,
    },
    theme::{ActiveDylanTheme as _, MetricAccent, Radii, Spacing, Typography},
};

use super::{CashFilter, CashView};

impl SearchableListItem for CashFilter {
    type Value = Self;

    fn title(&self) -> SharedString {
        self.label().into()
    }

    fn value(&self) -> &Self {
        self
    }
}

impl CashView {
    fn header(
        &self,
        active: Option<&CashSession>,
        ticket_value: i64,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let can_change_ticket_value = self.session.read(cx).can_change_ticket_value();
        v_flex()
            .gap(Spacing::SM)
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap(Spacing::SM)
                            .child(
                                Label::new("Caja")
                                    .text_size(Typography::PAGE_TITLE)
                                    .font_bold(),
                            )
                            .child(match active {
                                Some(cash) => StatusBadge::new(
                                    StatusTone::Success,
                                    format!("Abierta · Turno #{}", cash.id),
                                ),
                                None => StatusBadge::new(StatusTone::Neutral, "Caja cerrada"),
                            }),
                    )
                    .child(
                        AppButton::new("cash-ticket-value-toggle")
                            .secondary()
                            .small()
                            .icon(IconName::Settings)
                            .label(format!("Ticket · {}", format_soles(ticket_value)))
                            .disabled(self.pending || !can_change_ticket_value)
                            .tooltip("Cambiar el valor del ticket físico")
                            .on_click(cx.listener(|view, _, window, cx| {
                                if view.editing_ticket_value {
                                    view.editing_ticket_value = false;
                                    cx.notify();
                                } else {
                                    view.open_ticket_editor(window, cx);
                                }
                            })),
                    ),
            )
            .when(
                can_change_ticket_value && self.editing_ticket_value,
                |header| {
                    header.child(
                        cash_panel(cx).child(
                            h_flex()
                                .flex_wrap()
                                .items_end()
                                .gap(Spacing::SM)
                                .child(
                                    cash_field(
                                        "Valor del ticket físico (S/)",
                                        &self.ticket_value_input,
                                        cx,
                                    )
                                    .w(px(166.)),
                                )
                                .child(
                                    AppButton::new("cash-ticket-value-save")
                                        .small()
                                        .label("Guardar valor")
                                        .disabled(self.pending)
                                        .loading(self.pending)
                                        .on_click(cx.listener(|view, _, window, cx| {
                                            view.save_ticket_value(window, cx)
                                        })),
                                )
                                .child(
                                    AppButton::new("cash-ticket-value-cancel")
                                        .tertiary()
                                        .small()
                                        .label("Cancelar")
                                        .on_click(cx.listener(|view, _, _, cx| {
                                            view.editing_ticket_value = false;
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    Label::new("El valor se aplica a las próximas atenciones.")
                                        .text_size(Typography::XS)
                                        .text_color(palette.muted_foreground),
                                ),
                        ),
                    )
                },
            )
    }

    fn opening_panel(&self, cx: &Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        cash_panel(cx)
            .child(panel_title("Abrir turno"))
            .child(
                Label::new("Abre la caja para recibir tickets desde Atención automática.")
                    .text_size(Typography::SM)
                    .text_color(palette.muted_foreground),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .items_end()
                    .gap(Spacing::SM)
                    .child(
                        cash_field("Monto inicial (S/)", &self.opening_cash_input, cx).w(px(166.)),
                    )
                    .child(
                        AppButton::new("cash-open-submit")
                            .small()
                            .icon(IconName::Check)
                            .label("Abrir caja")
                            .disabled(self.pending)
                            .on_click(
                                cx.listener(|view, _, window, cx| {
                                    view.handle_open_cash(window, cx)
                                }),
                            ),
                    ),
            )
    }

    fn audit_panel(&self, cash: &CashSession, cx: &Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        cash_panel(cx)
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        v_flex()
                            .gap(Spacing::XXS)
                            .child(panel_title("Balance y cierre"))
                            .child(
                                Label::new(format!("{} · {}", cash.opened_at, cash.opened_by))
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        AppButton::new("cash-prefill-expected")
                            .tertiary()
                            .small()
                            .label("Usar valores esperados")
                            .on_click(
                                cx.listener(|view, _, window, cx| {
                                    view.prefill_expected(window, cx)
                                }),
                            ),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::MD)
                    .p(Spacing::SM)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .child(cash_stat(
                        "Efectivo esperado",
                        format_soles(cash.expected_cash_cents()),
                        cx,
                    ))
                    .child(cash_stat(
                        "Tickets esperados",
                        format!(
                            "{} · {}",
                            cash.total_tickets(),
                            format_soles(cash.total_ticket_amount_cents())
                        ),
                        cx,
                    )),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .gap(Spacing::SM)
                    .child(
                        cash_field("Efectivo contado (S/)", &self.counted_cash_input, cx)
                            .flex_1()
                            .min_w(px(132.)),
                    )
                    .child(
                        cash_field(
                            "Tickets contados (opcional)",
                            &self.counted_tickets_input,
                            cx,
                        )
                        .flex_1()
                        .min_w(px(154.)),
                    ),
            )
            .when_some(cash.last_audit.as_ref(), |panel, audit| {
                panel.child(
                    h_flex()
                        .flex_wrap()
                        .items_center()
                        .gap(Spacing::SM)
                        .child(audit_badge(
                            audit.is_balanced(),
                            "Último balance cuadrado",
                            "Último balance con diferencia",
                        ))
                        .child(
                            Label::new(format!(
                                "{} · Tickets {}/{} ({:+}) · Efectivo {}/{} (dif. {})",
                                audit.audited_at,
                                audit.counted_tickets,
                                audit.expected_tickets,
                                audit.ticket_diff,
                                format_soles(audit.counted_cash_cents),
                                format_soles(audit.expected_cash_cents),
                                format_soles(audit.cash_diff_cents)
                            ))
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                        ),
                )
            })
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_end()
                    .gap(Spacing::SM)
                    .child(
                        AppButton::new("cash-audit-submit")
                            .secondary()
                            .small()
                            .icon(IconName::Check)
                            .label("Registrar arqueo")
                            .disabled(self.pending)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.handle_audit_cash(window, cx)
                            })),
                    )
                    .child(
                        AppButton::new("cash-close-submit")
                            .destructive()
                            .small()
                            .label("Cerrar y archivar turno")
                            .disabled(self.pending)
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.handle_close_cash(window, cx)
                            })),
                    ),
            )
    }

    fn history(&self, ledger: &DiningLedger, cx: &Context<Self>) -> impl IntoElement {
        cash_panel(cx)
            .child(
                TabBar::new("cash-history-tabs")
                    .segmented()
                    .small()
                    .child(Tab::new().label("Movimientos"))
                    .child(Tab::new().label("Turnos cerrados"))
                    .selected_index(usize::from(self.show_closed_sessions))
                    .on_click(cx.listener(|view, index: &usize, _, cx| {
                        view.show_closed_sessions = *index == 1;
                        cx.notify();
                    })),
            )
            .child(if self.show_closed_sessions {
                closed_sessions(ledger.closed_cash_sessions(), cx).into_any_element()
            } else {
                self.movements(ledger, cx).into_any_element()
            })
    }

    fn movements(&self, ledger: &DiningLedger, cx: &Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let filter = self
            .filter
            .read(cx)
            .selected_value()
            .copied()
            .unwrap_or_default();
        let query = normalize_query(&self.search_input.read(cx).value());
        let selected = self.selected_pensioner.map(|id| {
            ledger
                .cash_movements()
                .iter()
                .rev()
                .find(|movement| movement.pensioner_id == Some(id))
                .and_then(|movement| {
                    movement
                        .pensioner_code
                        .as_ref()
                        .zip(movement.pensioner_name.as_ref())
                })
                .map_or_else(|| id.code(), |(code, name)| format!("{code} · {name}"))
        });
        let movements: Vec<_> = ledger
            .cash_movements()
            .iter()
            .rev()
            .filter(|movement| {
                if !filter.matches_kind(movement.kind)
                    || self
                        .selected_pensioner
                        .is_some_and(|id| movement.pensioner_id != Some(id))
                {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                let text = normalize_query(&format!(
                    "{} {} {} {} {} {}",
                    movement.id,
                    movement.kind.label(),
                    movement.description,
                    movement.operator_name,
                    movement.pensioner_code.as_deref().unwrap_or_default(),
                    movement.pensioner_name.as_deref().unwrap_or_default()
                ));
                query.split_whitespace().all(|term| text.contains(term))
            })
            .collect();

        v_flex()
            .gap(Spacing::SM)
            .child(
                h_flex()
                    .flex_wrap()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        div().flex_1().min_w(px(180.)).child(
                            Input::new(&self.search_input)
                                .small()
                                .cleanable(true)
                                .prefix(
                                    Icon::new(IconName::Search)
                                        .small()
                                        .text_color(palette.muted_foreground),
                                ),
                        ),
                    )
                    .child(
                        div().w(px(174.)).max_w_full().child(
                            Select::new(&self.filter)
                                .id("cash-movement-filter")
                                .small()
                                .w_full()
                                .accessibility_label("Tipo de movimiento"),
                        ),
                    ),
            )
            .when_some(selected, |body, person| {
                body.child(self.pensioner_filter(person, cx))
            })
            .child(if movements.is_empty() {
                let message = if ledger.cash_movements().is_empty() {
                    "Aún no hay movimientos de caja."
                } else {
                    "Sin movimientos para estos filtros."
                };
                empty_history(IconName::FileText, message, cx).into_any_element()
            } else {
                v_flex()
                    .child(
                        Label::new(format!("{} movimientos", movements.len()))
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                    )
                    .children(
                        movements
                            .into_iter()
                            .map(|movement| self.movement_row(movement, cx)),
                    )
                    .into_any_element()
            })
    }

    fn pensioner_filter(&self, person: String, cx: &Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        h_flex()
            .flex_wrap()
            .justify_between()
            .items_center()
            .gap(Spacing::XS)
            .px(Spacing::SM)
            .py(Spacing::XS)
            .rounded(Radii::MD)
            .bg(palette.info.tint)
            .child(
                Label::new(person)
                    .text_size(Typography::SM)
                    .font_medium()
                    .text_color(palette.info.foreground),
            )
            .child(
                AppButton::new("cash-clear-pensioner-filter")
                    .tertiary()
                    .xsmall()
                    .icon(IconName::Close)
                    .label("Quitar filtro")
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.selected_pensioner = None;
                        cx.notify();
                    })),
            )
    }

    fn movement_row(&self, movement: &CashMovement, cx: &Context<Self>) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let movement_id = movement.id.clone();
        let amount_color = match movement.kind {
            CashMovementKind::CashPayment => palette.success.foreground,
            CashMovementKind::TicketCredit => palette.info.foreground,
            CashMovementKind::Opening | CashMovementKind::Closing => palette.foreground,
        };
        h_flex()
            .w_full()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(Spacing::SM)
            .py(Spacing::SM)
            .border_t_1()
            .border_color(palette.border)
            .child(
                v_flex()
                    .flex_1()
                    .min_w(px(180.))
                    .gap(Spacing::XXS)
                    .child(
                        h_flex()
                            .flex_wrap()
                            .items_center()
                            .gap(Spacing::XS)
                            .child(movement_badge(movement.kind))
                            .child(
                                Label::new(movement.id.clone())
                                    .text_size(Typography::XS)
                                    .text_color(palette.muted_foreground),
                            ),
                    )
                    .child(
                        Label::new(movement.description.clone())
                            .text_size(Typography::SM)
                            .font_medium(),
                    )
                    .child(
                        Label::new(format!(
                            "{} · {} · Turno #{}",
                            movement.recorded_at, movement.operator_name, movement.cash_session_id
                        ))
                        .text_size(Typography::XS)
                        .text_color(palette.muted_foreground),
                    )
                    .when_some(
                        movement
                            .pensioner_id
                            .zip(movement.pensioner_code.as_ref())
                            .zip(movement.pensioner_name.as_ref()),
                        |row, ((id, code), name)| {
                            row.child(
                                div().child(
                                    AppButton::new(SharedString::from(format!(
                                        "cash-movement-person-{}",
                                        movement.id
                                    )))
                                    .tertiary()
                                    .xsmall()
                                    .label(format!("{code} · {name}"))
                                    .tooltip("Filtrar por este pensionista")
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.selected_pensioner = Some(id);
                                        view.filter.update(cx, |filter, cx| {
                                            filter.set_selected_value(&CashFilter::All, window, cx)
                                        });
                                        cx.notify();
                                    })),
                                ),
                            )
                        },
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap(Spacing::SM)
                    .child(
                        Label::new(format_soles(movement.amount_cents))
                            .text_size(Typography::BASE)
                            .font_bold()
                            .text_color(amount_color),
                    )
                    .child(
                        AppButton::new(SharedString::from(format!(
                            "cash-movement-receipt-{}",
                            movement.id
                        )))
                        .secondary()
                        .xsmall()
                        .icon(IconName::FileText)
                        .label("Recibo")
                        .on_click(cx.listener(
                            move |view, _, window, cx| {
                                view.open_receipt(movement_id.clone(), window, cx)
                            },
                        )),
                    ),
            )
    }

    fn render_receipt_modal(
        &self,
        movement: &CashMovement,
        current_balance_cents: Option<i64>,
        window: &Window,
        cx: &Context<Self>,
    ) -> impl IntoElement {
        let palette = cx.dylan_theme().palette;
        let viewport = window.viewport_size();
        let holder = match (&movement.pensioner_code, &movement.pensioner_name) {
            (Some(code), Some(name)) => format!("{name} ({code})"),
            (None, Some(name)) => name.clone(),
            _ => "Operación de caja general".to_string(),
        };

        let card = v_flex()
            .id("cash-receipt-card")
            .occlude()
            .w(px(460.0))
            .max_w(viewport.width - Spacing::MD * 2.0)
            .max_h(viewport.height - Spacing::LG * 2.0)
            .gap(Spacing::MD)
            .p(Spacing::LG)
            .rounded(Radii::XL)
            .bg(palette.surface)
            .border_1()
            .border_color(palette.border)
            .shadow(palette.brand.card_shadow())
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                h_flex()
                    .w_full()
                    .flex_wrap()
                    .flex_shrink_0()
                    .items_center()
                    .justify_between()
                    .gap(Spacing::SM)
                    .child(
                        v_flex().gap(Spacing::XXS).child(
                            Label::new(format!("Comprobante · {}", movement.id))
                                .text_size(Typography::LG)
                                .font_bold()
                                .text_color(palette.foreground),
                        ),
                    )
                    .child(movement_badge(movement.kind)),
            )
            .child(
                v_flex()
                    .id("cash-receipt-details")
                    .w_full()
                    .min_h_0()
                    .overflow_y_scroll()
                    .gap(Spacing::XS)
                    .p(Spacing::MD)
                    .rounded(Radii::MD)
                    .bg(palette.surface_subtle)
                    .border_1()
                    .border_color(palette.border)
                    .child(
                        Label::new(format!(
                            "Fecha / Registro: {} · Responsable: {}",
                            movement.recorded_at, movement.operator_name
                        ))
                        .text_size(Typography::XS)
                        .text_color(palette.muted_foreground),
                    )
                    .child(
                        Label::new(format!("Titular: {holder}"))
                            .text_size(Typography::SM)
                            .font_semibold()
                            .text_color(palette.foreground),
                    )
                    .child(
                        Label::new(format!("Concepto: {}", movement.description))
                            .text_size(Typography::SM)
                            .text_color(palette.foreground),
                    )
                    .when_some(
                        movement.ticket_count.zip(movement.ticket_unit_cents),
                        |this, (count, unit)| {
                            this.child(
                                Label::new(format!(
                                    "Detalle de tickets: {count} unidad(es) × {}",
                                    format_soles(unit)
                                ))
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                            )
                        },
                    )
                    .when_some(movement.consumption_cents, |this, consumed| {
                        this.child(
                            Label::new(format!("Consumo asociado: {}", format_soles(consumed)))
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                        )
                    })
                    .when_some(movement.resulting_balance_cents, |this, res_bal| {
                        this.child(
                            Label::new(format!(
                                "Saldo resultante en esa operación: {}",
                                format_balance_label(res_bal)
                            ))
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                        )
                    })
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .justify_between()
                            .pt(Spacing::SM)
                            .mt(Spacing::XS)
                            .border_t_1()
                            .border_color(palette.border)
                            .child(
                                Label::new("Importe:")
                                    .text_size(Typography::SM)
                                    .font_semibold()
                                    .text_color(palette.foreground),
                            )
                            .child(
                                Label::new(format_soles(movement.amount_cents))
                                    .text_size(Typography::LG)
                                    .font_bold()
                                    .text_color(palette.foreground),
                            ),
                    )
                    .when_some(current_balance_cents, |this, current| {
                        this.child(
                            Label::new(format!(
                                "Saldo actual del titular: {}",
                                format_balance_label(current)
                            ))
                            .text_size(Typography::XS)
                            .text_color(palette.muted_foreground),
                        )
                    }),
            )
            .child(
                h_flex().w_full().flex_shrink_0().justify_end().child(
                    AppButton::new("cash-receipt-close")
                        .primary()
                        .small()
                        .label("Cerrar")
                        .on_click(cx.listener(|view, _, _, cx| view.close_receipt(cx))),
                ),
            );

        deferred(
            div()
                .id("cash-receipt-overlay")
                .track_focus(&self.receipt_focus)
                .occlude()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(black().alpha(0.45))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|view, _, _, cx| view.close_receipt(cx)),
                )
                .capture_key_down(cx.listener(|view, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" {
                        cx.stop_propagation();
                        window.prevent_default();
                        view.close_receipt(cx);
                    }
                }))
                .child(card)
                .focus_trap("cash-receipt-focus", &self.receipt_focus),
        )
        .with_priority(2)
    }
}

fn format_balance_label(balance_cents: i64) -> String {
    if balance_cents > 0 {
        format!("A favor {}", format_soles(balance_cents))
    } else if balance_cents < 0 {
        format!("Deuda {}", format_soles(balance_cents.saturating_abs()))
    } else {
        "S/ 0.00 (Al día)".to_string()
    }
}

fn movement_badge(kind: CashMovementKind) -> StatusBadge {
    let tone = match kind {
        CashMovementKind::Opening => StatusTone::Info,
        CashMovementKind::Closing => StatusTone::Neutral,
        CashMovementKind::CashPayment => StatusTone::Success,
        CashMovementKind::TicketCredit => StatusTone::Info,
    };
    StatusBadge::new(tone, kind.label())
}

fn normalize_query(input: &str) -> String {
    input.trim().to_lowercase()
}

impl Render for CashView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let padding = if window.viewport_size().width < px(640.) {
            Spacing::SM
        } else {
            Spacing::LG
        };
        let store = self.store.read(cx);
        let load = store.cash_load();
        if load != DataLoad::Ready {
            return v_flex()
                .id("cash-initial")
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .items_center()
                .p(padding)
                .child(
                    v_flex()
                        .w_full()
                        .max_w(px(1080.))
                        .flex_shrink_0()
                        .gap(Spacing::MD)
                        .child(
                            Label::new("Caja")
                                .text_size(Typography::PAGE_TITLE)
                                .font_bold(),
                        )
                        .when(load == DataLoad::Loading, |body| {
                            body.child(
                                loading::region("cash-loading", "Cargando datos de Caja")
                                    .child(loading::line("shift-status", px(145.)))
                                    .child(loading::metrics(
                                        "cash-metrics-loading",
                                        "Cargando totales de Caja",
                                        2,
                                        cx,
                                    ))
                                    .child(cash_panel(cx).child(loading::rows(
                                        "cash-history-loading",
                                        "Cargando historial de Caja",
                                        3,
                                        cx,
                                    ))),
                            )
                        })
                        .when(load == DataLoad::Failed, |body| {
                            body.child(
                                Label::new("No se pudo cargar la Caja.")
                                    .text_color(cx.dylan_theme().palette.muted_foreground),
                            )
                        }),
                )
                .into_any_element();
        }
        let ledger = store.ledger();
        let active = ledger.active_cash();
        let cash_inflow = active.map_or_else(
            || ledger.total_cash_inflow_cents(),
            |cash| cash.cash_inflow_cents,
        );
        let receipt = self
            .receipt_movement_id
            .as_deref()
            .and_then(|id| ledger.find_cash_movement(id));

        v_flex()
            .id("cash-view")
            .relative()
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                v_flex()
                    .id("cash-scroll")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .overflow_y_scroll()
                    .items_center()
                    .p(padding)
                    .child(
                        v_flex()
                            .w_full()
                            .max_w(px(1080.))
                            .flex_shrink_0()
                            .gap(Spacing::MD)
                            .child(self.header(active, ledger.ticket_value_cents(), cx))
                            .when_some(self.feedback.clone(), |body, (tone, message)| {
                                body.child(
                                    match tone {
                                        StatusTone::Error => Alert::error("cash-feedback", message),
                                        StatusTone::Warning => {
                                            Alert::warning("cash-feedback", message)
                                        }
                                        StatusTone::Success => {
                                            Alert::success("cash-feedback", message)
                                        }
                                        _ => Alert::info("cash-feedback", message),
                                    }
                                    .small(),
                                )
                            })
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap(Spacing::SM)
                                    .child(
                                        MetricCard::new(
                                            MetricAccent::Neutral,
                                            "Fondo inicial",
                                            format_soles(
                                                active.map_or(0, |cash| cash.opening_cash_cents),
                                            ),
                                        )
                                        .icon(IconName::Inbox)
                                        .subtitle(
                                            if active.is_some() {
                                                "Turno actual"
                                            } else {
                                                "Sin turno activo"
                                            },
                                        ),
                                    )
                                    .child(
                                        MetricCard::new(
                                            MetricAccent::CashIn,
                                            "Efectivo ingresado",
                                            format_soles(cash_inflow),
                                        )
                                        .icon(IconName::CircleCheck)
                                        .subtitle(
                                            if active.is_some() {
                                                "Turno actual"
                                            } else {
                                                "Acumulado"
                                            },
                                        ),
                                    ),
                            )
                            .when(self.can_manage_cash(cx), |view| {
                                view.child(match active {
                                    Some(cash) => self.audit_panel(cash, cx).into_any_element(),
                                    None => self.opening_panel(cx).into_any_element(),
                                })
                            })
                            .child(self.history(ledger, cx)),
                    ),
            )
            .when_some(receipt, |body, movement| {
                let balance = movement
                    .pensioner_id
                    .and_then(|id| store.pensioner_history(id))
                    .and_then(|query| query.value.as_ref())
                    .map(|history| history.pensioner.balance_cents);
                body.child(self.render_receipt_modal(movement, balance, window, cx))
            })
            .into_any_element()
    }
}

fn cash_panel(cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .w_full()
        .gap(Spacing::SM)
        .p(Spacing::MD)
        .rounded(Radii::LG)
        .bg(palette.surface)
        .border_1()
        .border_color(palette.border)
}

fn panel_title(title: &'static str) -> Label {
    Label::new(title).text_size(Typography::LG).font_semibold()
}

fn cash_field(label: &'static str, input: &Entity<InputState>, cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .max_w_full()
        .gap(Spacing::XS)
        .child(
            Label::new(label)
                .text_size(Typography::XS)
                .font_medium()
                .text_color(palette.muted_foreground),
        )
        .child(Input::new(input).small().w_full())
}

fn cash_stat(label: &'static str, value: String, cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .flex_1()
        .min_w(px(115.))
        .gap(Spacing::XXS)
        .child(
            Label::new(label)
                .text_size(Typography::XS)
                .text_color(palette.muted_foreground),
        )
        .child(Label::new(value).text_size(Typography::SM).font_semibold())
}

fn audit_badge(balanced: bool, success: &'static str, warning: &'static str) -> StatusBadge {
    if balanced {
        StatusBadge::new(StatusTone::Success, success)
    } else {
        StatusBadge::new(StatusTone::Warning, warning)
    }
}

fn empty_history(icon: IconName, text: &'static str, cx: &App) -> Div {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .items_center()
        .gap(Spacing::SM)
        .py(Spacing::LG)
        .child(Icon::new(icon).small().text_color(palette.muted_foreground))
        .child(
            Label::new(text)
                .text_size(Typography::SM)
                .text_color(palette.muted_foreground),
        )
}

fn closed_sessions(sessions: &[ClosedCashSession], cx: &App) -> impl IntoElement {
    let palette = cx.dylan_theme().palette;
    v_flex()
        .when(sessions.is_empty(), |list| {
            list.child(empty_history(
                IconName::Calendar,
                "Aún no se ha cerrado ningún turno.",
                cx,
            ))
        })
        .children(sessions.iter().rev().map(|cash| {
            v_flex()
                .gap(Spacing::SM)
                .py(Spacing::SM)
                .border_t_1()
                .border_color(palette.border)
                .child(
                    h_flex()
                        .flex_wrap()
                        .justify_between()
                        .items_center()
                        .gap(Spacing::SM)
                        .child(
                            Label::new(format!("Turno #{}", cash.id))
                                .text_size(Typography::BASE)
                                .font_semibold(),
                        )
                        .child(audit_badge(
                            cash.audit.is_balanced(),
                            "Cuadrado",
                            "Con diferencia",
                        )),
                )
                .child(
                    Label::new(format!(
                        "{} → {} · {} movimientos",
                        cash.opened_at, cash.closed_at, cash.movement_count
                    ))
                    .text_size(Typography::XS)
                    .text_color(palette.muted_foreground),
                )
                .child(
                    h_flex()
                        .flex_wrap()
                        .gap(Spacing::SM)
                        .child(cash_stat(
                            "Efectivo contado / esperado",
                            format!(
                                "{} / {}",
                                format_soles(cash.audit.counted_cash_cents),
                                format_soles(cash.expected_cash_cents)
                            ),
                            cx,
                        ))
                        .child(cash_stat(
                            "Tickets contados / esperados",
                            format!(
                                "{} / {} · {}",
                                cash.audit.counted_tickets,
                                cash.expected_tickets,
                                format_soles(cash.expected_ticket_amount_cents)
                            ),
                            cx,
                        )),
                )
                .child(
                    Label::new(format!("Cerrado por {}", cash.closed_by))
                        .text_size(Typography::SM)
                        .text_color(palette.muted_foreground),
                )
        }))
}
