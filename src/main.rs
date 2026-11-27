#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod app;
mod components;
mod features;
mod services;
mod state;
mod theme;

use gpui_kit::{component::TitleBar, *};

fn main() {
    velopack::VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .run();

    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        gpui_kit::component::set_locale("es");
        theme::install_themes(cx);
        components::brand::init(cx);
        cx.bind_keys([
            KeyBinding::new("f5", app::CheckForUpdates, None),
            KeyBinding::new("ctrl-k", app::FocusGlobalSearch, None),
        ]);

        let mut options = TitleBar::window_options();
        #[cfg(target_os = "linux")]
        {
            options.window_decorations = Some(WindowDecorations::Client);
            let icon = image::load_from_memory_with_format(
                include_bytes!("../assets/app.png"),
                image::ImageFormat::Png,
            )
            .expect("El icono de Dylan debe ser un PNG válido");
            options.icon = Some(std::sync::Arc::new(icon.into_rgba8()));
        }
        options.window_min_size = Some(size(px(360.0), px(360.0)));
        if let Some(titlebar) = &mut options.titlebar {
            titlebar.title = Some("Comedor Dylan".into());
        }

        open_window(options, cx, |window, cx| {
            let shell = cx.new(|cx| app::DesktopApp::new(window, cx));
            let shell_for_close = shell.clone();
            window.on_window_should_close(cx, move |_, cx| shell_for_close.read(cx).can_close(cx));
            shell
        })
        .expect("No se pudo abrir la ventana");
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
