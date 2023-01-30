use std::collections::BTreeMap;

// pub struct AppSettings {
// }

// pub struct SettingsTab {
//     //
// }

struct SettingsHandler {
    name: String,
    show: Box<dyn Fn(&mut egui::Ui) + Send + Sync + 'static>,
    // validate: Option<Box<dyn Fn() -> bool + Send + Sync + 'static>;
}

#[derive(Default)]
pub struct SettingsWindow {
    handlers: Vec<SettingsHandler>,
}

impl SettingsWindow {
    pub fn register_widget(
        &mut self,
        name: &str,
        show: impl Fn(&mut egui::Ui) + Send + Sync + 'static,
    ) {
        let h = SettingsHandler {
            name: name.to_string(),
            show: Box::new(show),
        };

        self.handlers.push(h);
    }

    pub fn show(&self, ctx: &egui::Context) {
        egui::Window::new("Settings").show(ctx, |ui| {
            for h in self.handlers.iter() {
                ui.collapsing(&h.name, &h.show);
            }
        });
    }
}
