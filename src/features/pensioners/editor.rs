use std::cell::Cell;

use gpui_kit::{
    component::{
        StyledExt as _, WindowExt as _,
        checkbox::Checkbox,
        h_flex,
        input::{Input, InputState},
        label::Label,
        v_flex,
    },
    prelude::FluentBuilder as _,
    *,
};

use crate::{
    components::button::AppButton,
    theme::{ActiveDylanTheme as _, Radii, Spacing, Typography},
};

use super::{
    PensionersEvent, PensionersView,
    enrollment::EnrollmentModal,
    model::{PensionerDraft, PensionerId},
};

#[derive(Default)]
struct EditorState {
    enroll: bool,
    consent: bool,
    error: Option<&'static str>,
}

impl PensionersView {
    pub(super) fn open_editor(
        &self,
        id: Option<PensionerId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let permissions = self.session.read(cx).pensioner_permissions();
        if (if id.is_some() {
            !permissions.update
        } else {
            !permissions.create
        }) || window.has_active_dialog(cx)
        {
            return;
        }

        let (draft, can_enroll_profile, temporary) = match id {
            Some(id) => {
                let Some(pensioner) = self.store.read(cx).ledger().find_pensioner(id) else {
                    return;
                };
                (
                    PensionerDraft::from(pensioner),
                    pensioner.can_enroll_fingerprint(),
                    pensioner.temporary_profile,
                )
            }
            None => (PensionerDraft::default(), true, true),
        };
        let fields = [
            ("Nombre completo", draft.full_name),
            ("DNI", draft.dni),
            ("Celular", draft.phone),
        ]
        .map(|(label, value)| {
            (
                label,
                cx.new(|cx| InputState::new(window, cx).default_value(value)),
            )
        });
        let state = cx.new(|_| EditorState::default());
        let initial_focus = Cell::new(true);
        let store = self.store.clone();
        let session = self.session.clone();
        let reader = self.reader.clone();
        let owner = cx.weak_entity();

        window.open_dialog(cx, move |dialog, window, cx| {
            if initial_focus.replace(false) {
                fields[0].1.read(cx).focus_handle(cx).focus(window, cx);
            }
            let inputs = fields.each_ref().map(|(_, input)| input.clone());
            let save_state = state.clone();
            let toggle_enroll = state.clone();
            let toggle_consent = state.clone();
            let store = store.clone();
            let session = session.clone();
            let reader = reader.clone();
            let owner = owner.clone();
            let palette = cx.dylan_theme().palette;
            let can_enroll = session.read(cx).pensioner_permissions().enroll && can_enroll_profile;
            let reader_eligible = EnrollmentModal::can_use_reader(temporary, reader.read(cx));
            let show_enrollment = can_enroll && reader_eligible;
            let form = state.read(cx);
            dialog
                .title(if id.is_some() {
                    "Editar pensionista"
                } else {
                    "Nuevo pensionista"
                })
                .width(px(440.))
                .max_w(window.viewport_size().width - Spacing::MD * 2.)
                .overlay_closable(false)
                .child(
                    v_flex()
                        .id("pensioner-profile-form")
                        .max_h((window.viewport_size().height - px(180.)).max(px(100.)))
                        .overflow_y_scroll()
                        .p(Spacing::XS)
                        .gap(Spacing::SM)
                        .children(fields.iter().map(|(label, input)| {
                            v_flex()
                                .w_full()
                                .flex_shrink_0()
                                .gap(Spacing::XS)
                                .child(Label::new(*label).text_size(Typography::SM).font_medium())
                                .child(Input::new(input).w_full().rounded(Radii::MD).bg(palette.surface))
                        }))
                        .when(show_enrollment, |body| {
                            body.child(
                                v_flex().flex_shrink_0().gap(Spacing::XS).pt(Spacing::SM)
                                    .border_t_1().border_color(palette.border)
                                    .child(Checkbox::new("register-pensioner-fingerprint")
                                        .label("Registrar huella después de guardar")
                                        .checked(form.enroll)
                                        .on_change(move |checked, window, cx| {
                                            toggle_enroll.update(cx, |state, cx| {
                                                state.enroll = *checked;
                                                state.consent = false;
                                                state.error = None;
                                                cx.notify();
                                            });
                                            window.refresh();
                                        }))
                                    .when(form.enroll, |option| {
                                        option.child(Checkbox::new("pensioner-fingerprint-consent")
                                            .label("Consentimiento informado otorgado")
                                            .checked(form.consent)
                                            .on_change(move |checked, window, cx| {
                                                toggle_consent.update(cx, |state, cx| {
                                                    state.consent = *checked;
                                                    state.error = None;
                                                    cx.notify();
                                                });
                                                window.refresh();
                                            }))
                                    }),
                            )
                        })
                        .when(can_enroll && !reader_eligible, |body| {
                            body.child(Label::new("Los perfiles temporales permiten registrar la huella con el simulador.")
                                .text_size(Typography::XS).text_color(palette.muted_foreground))
                        })
                        .children(form.error.map(|message| {
                            Label::new(message).text_color(palette.error.foreground)
                        })),
                )
                .footer(
                    h_flex()
                        .gap(Spacing::SM)
                        .justify_end()
                        .child(
                            AppButton::new("cancel-pensioner-editor")
                                .secondary()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(AppButton::new("save-pensioner").label("Guardar").on_click(
                            move |_, window, cx| {
                                let permissions = session.read(cx).pensioner_permissions();
                                if !(if id.is_some() {
                                    permissions.update
                                } else {
                                    permissions.create
                                }) {
                                    window.close_dialog(cx);
                                    return;
                                }
                                let form = save_state.read(cx);
                                let enroll = form.enroll && permissions.enroll && can_enroll_profile
                                    && EnrollmentModal::can_use_reader(temporary, reader.read(cx));
                                if enroll && !form.consent {
                                    show_error(&save_state, "Confirma el consentimiento para registrar la huella.", window, cx);
                                    return;
                                }
                                let [full_name, dni, phone] = inputs
                                    .each_ref()
                                    .map(|input| input.read(cx).value().to_string());
                                let draft = PensionerDraft {
                                    full_name,
                                    dni,
                                    phone,
                                };
                                match store
                                    .update(cx, |store, cx| store.save_pensioner(id, draft, cx))
                                {
                                    Ok(saved_id) => {
                                        window.close_dialog(cx);
                                        if enroll {
                                            let owner = owner.clone();
                                            window.defer(cx, move |_, cx| {
                                                let _ = owner.update(cx, |_, cx| {
                                                    cx.emit(PensionersEvent::OpenEnrollment(saved_id));
                                                });
                                            });
                                        }
                                    }
                                    Err(message) => show_error(&save_state, message, window, cx),
                                }
                            },
                        )),
                )
        });
    }

    pub(super) fn confirm_status(&self, id: PensionerId, window: &mut Window, cx: &mut App) {
        if !self.session.read(cx).pensioner_permissions().update || window.has_active_dialog(cx) {
            return;
        }
        let Some(pensioner) = self.store.read(cx).ledger().find_pensioner(id) else {
            return;
        };
        let active = !pensioner.active;
        let action = if active { "Activar" } else { "Desactivar" };
        let title = format!("{action} pensionista");
        let prompt = format!(
            "¿Quieres {} a {}?",
            action.to_lowercase(),
            pensioner.full_name
        );
        let store = self.store.clone();
        let session = self.session.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let store = store.clone();
            let session = session.clone();
            let confirm = AppButton::new("confirm-pensioner-status").label(action);
            let confirm = if active {
                confirm.primary()
            } else {
                confirm.destructive()
            };
            dialog
                .title(title.clone())
                .child(Label::new(prompt.clone()))
                .footer(
                    h_flex()
                        .gap(Spacing::SM)
                        .justify_end()
                        .child(
                            AppButton::new("cancel-pensioner-status")
                                .secondary()
                                .label("Cancelar")
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(confirm.on_click(move |_, window, cx| {
                            if session.read(cx).pensioner_permissions().update {
                                store.update(cx, |store, cx| {
                                    store.set_pensioner_active(id, active, cx)
                                });
                            }
                            window.close_dialog(cx);
                        })),
                )
        });
    }
}

fn show_error(
    state: &Entity<EditorState>,
    message: &'static str,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, cx| {
        state.error = Some(message);
        cx.notify();
    });
    window.refresh();
}
