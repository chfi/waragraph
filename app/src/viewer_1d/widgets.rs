use tokio::sync::RwLock;

use std::sync::Arc;

use crate::app::{
    settings_menu::{SettingsUiContext, SettingsUiResponse, SettingsWidget},
    SharedState,
};

pub struct VisualizationModesWidget {
    pub(super) shared: SharedState,
    pub(super) active_viz_data_key: Arc<RwLock<String>>,
}

impl SettingsWidget for VisualizationModesWidget {
    fn show(
        &mut self,
        ui: &mut egui::Ui,
        settings_ctx: &SettingsUiContext,
    ) -> SettingsUiResponse {
        let mut current_key = self.active_viz_data_key.blocking_write();

        let mut path_data_sources = self
            .shared
            .graph_data_cache
            .path_data_source_names()
            .collect::<Vec<_>>();
        path_data_sources.sort();

        // let mut current_key = self.active_viz_data_key.clone();

        let resp = ui.horizontal(|ui| {
            for key in path_data_sources {
                if ui
                    .add_enabled(
                        key != current_key.as_str(),
                        egui::Button::new(key),
                    )
                    .clicked()
                {
                    *current_key = key.to_string();
                }
            }
        });

        // self.active_viz_data_key = current_key;
        SettingsUiResponse {
            response: resp.response,
        }
    }
}
