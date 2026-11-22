mod app;
mod services;

use gpui_kit::{AppContext as _, WindowOptions, application, assets, init, open_window};

fn main() {
    velopack::VelopackApp::build()
        .set_auto_apply_on_startup(false)
        .run();

    application().with_assets(assets::Assets).run(|cx| {
        init(cx);

        open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(app::DesktopApp::new)
        })
        .expect("No se pudo abrir la ventana");
    });
}
