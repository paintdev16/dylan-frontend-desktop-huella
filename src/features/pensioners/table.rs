use std::collections::HashSet;

use gpui_kit::{
    component::{
        IconName, StyledExt as _, h_flex,
        label::Label,
        table::{Column, ColumnSort, TableDelegate, TableState},
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::button::AppButton,
    state::session::PensionerPermissions,
    theme::{ActiveDylanTheme as _, Spacing, Typography},
};

use super::{
    PensionersEvent, PensionersView, account_status, fingerprint_status,
    model::{Pensioner, PensionerId, format_soles},
};

pub(super) struct PensionersTable {
    owner: WeakEntity<PensionersView>,
    rows: Vec<Pensioner>,
    permissions: PensionerPermissions,
    sort: (usize, ColumnSort),
    visible_balances: HashSet<(PensionerId, usize)>,
}

impl PensionersTable {
    pub fn new(owner: WeakEntity<PensionersView>, rows: Vec<Pensioner>) -> Self {
        Self {
            owner,
            rows,
            permissions: PensionerPermissions::default(),
            sort: (0, ColumnSort::Default),
            visible_balances: HashSet::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn set_rows(&mut self, rows: Vec<Pensioner>, permissions: PensionerPermissions) {
        self.rows = rows;
        self.permissions = permissions;
        self.sort_rows();
    }

    pub fn hide_balances(&mut self) {
        self.visible_balances.clear();
    }

    fn sort_rows(&mut self) {
        let (column, direction) = self.sort;
        self.rows.sort_by(|left, right| {
            let order = match (column, direction) {
                (_, ColumnSort::Default) => left.id.cmp(&right.id),
                (1, _) => left.phone.cmp(&right.phone),
                (2, _) => left.active.cmp(&right.active),
                (3, _) => left.fingerprint_enrolled.cmp(&right.fingerprint_enrolled),
                _ => left
                    .full_name
                    .to_lowercase()
                    .cmp(&right.full_name.to_lowercase()),
            }
            .then(left.id.cmp(&right.id));
            match direction {
                ColumnSort::Descending => order.reverse(),
                _ => order,
            }
        });
    }

    fn actions(&self, pensioner: &Pensioner) -> impl IntoElement {
        let id = pensioner.id;
        let owner = self.owner.clone();
        let edit_owner = owner.clone();
        let status_owner = owner.clone();
        let enrollment_owner = owner.clone();
        h_flex()
            .gap(Spacing::XS)
            .child(
                AppButton::new(("pensioner-detail", id.0 as usize))
                    .secondary()
                    .xsmall()
                    .label("Detalle")
                    .on_click(move |_, _, cx| {
                        let _ = owner.update(cx, |_, cx| cx.emit(PensionersEvent::Selected(id)));
                    }),
            )
            .when(self.permissions.update, |actions| {
                actions
                    .child(
                        AppButton::new(("pensioner-edit", id.0 as usize))
                            .tertiary()
                            .xsmall()
                            .label("Editar")
                            .on_click(move |_, window, cx| {
                                let _ = edit_owner
                                    .update(cx, |view, cx| view.open_editor(Some(id), window, cx));
                            }),
                    )
                    .child(
                        AppButton::new(("pensioner-status", id.0 as usize))
                            .tertiary()
                            .xsmall()
                            .label(if pensioner.active {
                                "Desactivar"
                            } else {
                                "Activar"
                            })
                            .on_click(move |_, window, cx| {
                                let _ = status_owner
                                    .update(cx, |view, cx| view.confirm_status(id, window, cx));
                            }),
                    )
            })
            .when(
                self.permissions.enroll && pensioner.can_enroll_fingerprint(),
                |actions| {
                    actions.child(
                        AppButton::new(("pensioner-fingerprint", id.0 as usize))
                            .tertiary()
                            .xsmall()
                            .label("Huella")
                            .tooltip("Registrar huella")
                            .on_click(move |_, _, cx| {
                                let _ = enrollment_owner.update(cx, |_, cx| {
                                    cx.emit(PensionersEvent::OpenEnrollment(id))
                                });
                            }),
                    )
                },
            )
    }

    fn balance_cell(
        &self,
        pensioner: &Pensioner,
        column: usize,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let key = (pensioner.id, column);
        let visible = self.visible_balances.contains(&key);
        let cents = if column == 4 {
            pensioner.credit_cents()
        } else {
            pensioner.debt_cents()
        };
        let tone = if column == 4 {
            cx.dylan_theme().palette.success
        } else {
            cx.dylan_theme().palette.error
        };
        h_flex()
            .items_center()
            .gap(Spacing::XS)
            .child(
                Label::new(if visible {
                    format_soles(cents)
                } else {
                    "••••".to_owned()
                })
                .text_color(tone.foreground),
            )
            .child(
                AppButton::new(if column == 4 {
                    "show-credit"
                } else {
                    "show-debt"
                })
                .xsmall()
                .tertiary()
                .icon(if visible {
                    IconName::EyeOff
                } else {
                    IconName::Eye
                })
                .tooltip(if visible {
                    "Ocultar importe"
                } else {
                    "Mostrar importe"
                })
                .on_click(cx.listener(move |table, _, _, cx| {
                    let balances = &mut table.delegate_mut().visible_balances;
                    if !balances.remove(&key) {
                        balances.insert(key);
                    }
                    cx.notify();
                })),
            )
    }
}

impl TableDelegate for PensionersTable {
    fn columns_count(&self, _: &App) -> usize {
        7
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, index: usize, _: &App) -> Column {
        let (key, title, width) = [
            ("name", "Nombre completo", 215.),
            ("phone", "Celular", 116.),
            ("active", "Estado", 100.),
            ("fingerprint", "Huella", 110.),
            ("credit", "A favor", 110.),
            ("debt", "En contra", 110.),
            ("actions", "Acciones", 250.),
        ][index];
        let column = Column::new(key, title).width(px(width));
        if index < 4 { column.sortable() } else { column }
    }

    fn perform_sort(
        &mut self,
        column: usize,
        sort: ColumnSort,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) {
        self.sort = (column, sort);
        self.sort_rows();
        cx.notify();
    }

    fn render_td(
        &mut self,
        row: usize,
        column: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let pensioner = &self.rows[row];
        let muted = cx.dylan_theme().palette.muted_foreground;
        match column {
            0 => v_flex()
                .min_w_0()
                .child(
                    Label::new(pensioner.full_name.clone())
                        .font_medium()
                        .truncate(),
                )
                .child(
                    Label::new(pensioner.code())
                        .text_size(Typography::XS)
                        .text_color(muted),
                )
                .into_any_element(),
            1 => {
                Label::new(pensioner.phone.as_deref().unwrap_or("—").to_owned()).into_any_element()
            }
            2 => account_status(pensioner).into_any_element(),
            3 => fingerprint_status(pensioner).into_any_element(),
            4 | 5 => self.balance_cell(pensioner, column, cx).into_any_element(),
            _ => self.actions(pensioner).into_any_element(),
        }
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        h_flex().size_full().justify_center().child(
            Label::new("No se encontraron pensionistas.")
                .text_color(cx.dylan_theme().palette.muted_foreground),
        )
    }

    fn cell_label(&self, row: usize, column: usize, _: &App) -> Option<SharedString> {
        let pensioner = self.rows.get(row)?;
        let text = match column {
            0 => format!("{} ({})", pensioner.full_name, pensioner.code()),
            1 => pensioner.phone.clone().unwrap_or_default(),
            2 => if pensioner.active {
                "Activo"
            } else {
                "Inactivo"
            }
            .to_owned(),
            3 => if pensioner.fingerprint_enrolled {
                "Huella registrada"
            } else {
                "Huella pendiente"
            }
            .to_owned(),
            _ => return None,
        };
        Some(text.into())
    }
}
