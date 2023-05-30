use crossbeam::atomic::AtomicCell;
use std::sync::Arc;

use crate::app::settings_menu;

pub struct Config {
    // use_linear_sampler: Arc<AtomicCell<bool>>,
    pub(super) filter_path_list_by_visibility: Arc<AtomicCell<bool>>,
}

pub struct ConfigWidget {
    cfg: Config,
}

impl settings_menu::SettingsWidget for SettingsWidget {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings_ctx: &settings_menu::SettingsUiContext,
    ) -> settings_menu::SettingsUiResponse {
        todo!()
    }
}
