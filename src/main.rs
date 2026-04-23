mod app;
mod constants;
mod event_protocol;
mod event_server;
mod fonts;
mod model;
mod shell;

use fonts::setup_custom_fonts;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 820.0])
            .with_drag_and_drop(true)
            .with_transparent(false)
            .with_decorations(false),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    eframe::run_native(
        "Node Graph MVP (egui terminal)",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::light());
            setup_custom_fonts(&cc.egui_ctx);
            Ok(Box::new(app::GraphApp::new(cc)))
        }),
    )
}
