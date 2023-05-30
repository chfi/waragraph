use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

use crate::app::settings_menu;

#[derive(Debug, Clone)]
pub struct Config {
    pub(super) show_annotation_labels: Arc<AtomicCell<bool>>,
}

impl std::default::Default for Config {
    fn default() -> Self {
        Self {
            show_annotation_labels: Arc::new(true.into()),
        }
    }
}

pub struct ConfigWidget {
    pub(super) cfg: Config,
}

impl settings_menu::SettingsWidget for ConfigWidget {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        _settings_ctx: &settings_menu::SettingsUiContext,
    ) -> settings_menu::SettingsUiResponse {
        let mut filter_paths = self.cfg.show_annotation_labels.load();
        let response =
            ui.checkbox(&mut filter_paths, "Display annotation labels");
        self.cfg.show_annotation_labels.store(filter_paths);

        settings_menu::SettingsUiResponse { response }
    }
}
