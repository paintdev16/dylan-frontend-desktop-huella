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
    model::{PensionerDraft, PensionerId},
};

#[derive(Default)]
struct EditorState {
    enroll: bool,
    consent: bool,
    saving: bool,
    error: Option<SharedString>,
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
            None => (PensionerDraft::default(), true, false),
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
            let owner = owner.clone();
            let palette = cx.dylan_theme().palette;
            let can_enroll = session.read(cx).pensioner_permissions().enroll && can_enroll_profile;
            let show_enrollment = can_enroll && !temporary;
            let form = state.read(cx);
            let saving = form.saving;
            let submit = move |window: &mut Window, cx: &mut App| {
                let permissions = session.read(cx).pensioner_permissions();
                if !(if id.is_some() {
                    permissions.update
                } else {
                    permissions.create
                }) {
                    show_error(
                        &save_state,
                        "Ya no tienes permiso para guardar este perfil.",
                        window,
                        cx,
                    );
                    return;
                }
                let form = save_state.read(cx);
                if form.saving {
                    return;
                }
                let enroll = form.enroll && permissions.enroll && can_enroll_profile && !temporary;
                if enroll && !form.consent {
                    show_error(
                        &save_state,
                        "Confirma el consentimiento para registrar la huella.",
                        window,
                        cx,
                    );
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
                let draft = match draft.validated() {
                    Ok(draft) => draft,
                    Err(error) => {
                        show_error(&save_state, error, window, cx);
                        return;
                    }
                };
                save_state.update(cx, |state, cx| {
                    state.saving = true;
                    state.error = None;
                    cx.notify();
                });
                window.refresh();
                let window = window.window_handle();
                let state = save_state.downgrade();
                let owner = owner.clone();
                store.update(cx, |store, cx| {
                    store.request(
                        move |api| api.save_pensioner(id, &draft),
                        move |store, result, cx| {
                            let result = result.map(|pensioner| {
                                let id = pensioner.id;
                                store.update_pensioner(pensioner, cx);
                                id
                            });
                            let Some(state) = state.upgrade() else {
                                return;
                            };
                            let _ = window.update(cx, |_, window, cx| {
                                state.update(cx, |state, cx| {
                                    state.saving = false;
                                    cx.notify();
                                });
                                match result {
                                    Ok(saved_id) => {
                                        window.close_dialog(cx);
                                        if enroll {
                                            window.defer(cx, move |_, cx| {
                                                let _ = owner.update(cx, |_, cx| {
                                                    cx.emit(PensionersEvent::OpenEnrollment(
                                                        saved_id,
                                                    ))
                                                });
                                            });
                                        }
                                    }
                                    Err(error) => {
                                        show_error(&state, error.user_message(), window, cx)
                                    }
                                }
                            });
                        },
                        cx,
                    )
                });
            };
            let submit_with_enter = submit.clone();
            dialog
                .title(if id.is_some() {
                    "Editar pensionista"
                } else {
                    "Nuevo pensionista"
                })
                .width(px(440.))
                .max_w(window.viewport_size().width - Spacing::MD * 2.)
                .overlay_closable(false)
                .close_button(!saving)
                .on_ok({
                    let inputs = fields.each_ref().map(|(_, input)| input.clone());
                    move |_, window, cx| {
                        if inputs
                            .iter()
                            .any(|input| input.read(cx).focus_handle(cx).is_focused(window))
                        {
                            submit_with_enter(window, cx);
                        } else {
                            cx.propagate();
                        }
                        false
                    }
                })
                .on_cancel({
                    let state = state.clone();
                    move |_, _, cx| !state.read(cx).saving
                })
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
                                .child(
                                    Input::new(input)
                                        .w_full()
                                        .rounded(Radii::MD)
                                        .bg(palette.surface),
                                )
                        }))
                        .when(show_enrollment, |body| {
                            body.child(
                                v_flex()
                                    .flex_shrink_0()
                                    .gap(Spacing::XS)
                                    .pt(Spacing::SM)
                                    .border_t_1()
                                    .border_color(palette.border)
                                    .child(
                                        Checkbox::new("register-pensioner-fingerprint")
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
                                            }),
                                    )
                                    .when(form.enroll, |option| {
                                        option.child(
                                            Checkbox::new("pensioner-fingerprint-consent")
                                                .label("Consentimiento informado otorgado")
                                                .checked(form.consent)
                                                .on_change(move |checked, window, cx| {
                                                    toggle_consent.update(cx, |state, cx| {
                                                        state.consent = *checked;
                                                        state.error = None;
                                                        cx.notify();
                                                    });
                                                    window.refresh();
                                                }),
                                        )
                                    }),
                            )
                        })
                        .when(can_enroll && temporary, |body| {
                            body.child(
                                Label::new(
                                    "El enrolamiento requiere un perfil con un código persistente.",
                                )
                                .text_size(Typography::XS)
                                .text_color(palette.muted_foreground),
                            )
                        })
                        .children(form.error.clone().map(|message| {
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
                                .disabled(saving)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(
                            AppButton::new("save-pensioner")
                                .label("Guardar")
                                .disabled(saving)
                                .loading(saving)
                                .on_click(move |_, window, cx| submit(window, cx)),
                        ),
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
        let state = cx.new(|_| EditorState::default());
        window.open_dialog(cx, move |dialog, _, cx| {
            let store = store.clone();
            let session = session.clone();
            let confirm = AppButton::new("confirm-pensioner-status").label(action);
            let confirm = if active {
                confirm.primary()
            } else {
                confirm.destructive()
            };
            let state = state.clone();
            let form = state.read(cx);
            let saving = form.saving;
            let error = form.error.clone();
            dialog
                .title(title.clone())
                .overlay_closable(false)
                .close_button(!saving)
                .on_ok(|_, _, cx| {
                    // Delega Enter al botón enfocado, incluso si la acción es destructiva.
                    cx.propagate();
                    false
                })
                .on_cancel({
                    let state = state.clone();
                    move |_, _, cx| !state.read(cx).saving
                })
                .child(Label::new(prompt.clone()))
                .children(error.map(Label::new))
                .footer(
                    h_flex()
                        .gap(Spacing::SM)
                        .justify_end()
                        .child(
                            AppButton::new("cancel-pensioner-status")
                                .secondary()
                                .label("Cancelar")
                                .disabled(saving)
                                .on_click(|_, window, cx| window.close_dialog(cx)),
                        )
                        .child(confirm.disabled(saving).loading(saving).on_click(
                            move |_, window, cx| {
                                if !session.read(cx).pensioner_permissions().update
                                    || state.read(cx).saving
                                {
                                    return;
                                }
                                state.update(cx, |state, cx| {
                                    state.saving = true;
                                    state.error = None;
                                    cx.notify();
                                });
                                window.refresh();
                                let window = window.window_handle();
                                let state = state.downgrade();
                                store.update(cx, |store, cx| {
                                    store.request(
                                        move |api| api.set_pensioner_active(id, active),
                                        move |store, result, cx| {
                                            if result.is_ok() {
                                                store.set_pensioner_active(id, active, cx);
                                            }
                                            let Some(state) = state.upgrade() else {
                                                return;
                                            };
                                            let _ = window.update(cx, |_, window, cx| {
                                                state.update(cx, |state, cx| {
                                                    state.saving = false;
                                                    cx.notify();
                                                });
                                                match result {
                                                    Ok(_) => window.close_dialog(cx),
                                                    Err(error) => show_error(
                                                        &state,
                                                        error.user_message(),
                                                        window,
                                                        cx,
                                                    ),
                                                }
                                            });
                                        },
                                        cx,
                                    )
                                });
                            },
                        )),
                )
        });
    }
}

fn show_error(
    state: &Entity<EditorState>,
    message: impl Into<SharedString>,
    window: &mut Window,
    cx: &mut App,
) {
    state.update(cx, |state, cx| {
        state.error = Some(message.into());
        cx.notify();
    });
    window.refresh();
}
