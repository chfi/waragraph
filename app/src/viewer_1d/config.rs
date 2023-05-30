use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

use crate::app::settings_menu;

#[derive(Debug, Clone)]
pub struct Config {
    // use_linear_sampler: Arc<AtomicCell<bool>>,
    pub(super) filter_path_list_by_visibility: Arc<AtomicCell<bool>>,
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
        let mut filter_paths = self.cfg.filter_path_list_by_visibility.load();
        let response =
            ui.checkbox(&mut filter_paths, "Filter paths by visibility");
        self.cfg.filter_path_list_by_visibility.store(filter_paths);

        settings_menu::SettingsUiResponse { response }
    }
}
