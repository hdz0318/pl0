use eframe::egui;
use pl0::gui::Pl0Gui;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_title("PL/0 Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "PL/0 Studio",
        native_options,
        Box::new(|cc| Ok(Box::new(Pl0Gui::new(cc)))),
    )
}
