use crossbeam::atomic::AtomicCell;
use palette::convert::IntoColorUnclamped;
use tokio::sync::RwLock;

use std::sync::Arc;

use crate::app::{
    settings_menu::{SettingsUiContext, SettingsUiResponse, SettingsWidget},
    SharedState,
};

pub struct VisualizationModesWidget {
    pub(super) shared: SharedState,
    pub(super) active_viz_data_key: Arc<RwLock<String>>,
    pub(super) use_linear_sampler: Arc<AtomicCell<bool>>,
}

impl VisualizationModesWidget {
    pub fn new(
        shared: SharedState,
        active_viz_data_key: Arc<RwLock<String>>,
        use_linear_sampler: Arc<AtomicCell<bool>>,
    ) -> Self {
        Self {
            shared,
            active_viz_data_key,
            use_linear_sampler,
        }
    }
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

        let resp = ui.vertical(|ui| {
            let data_sources = ui.horizontal(|ui| {
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

            let sampler = {
                let mut use_linear = self.use_linear_sampler.load();
                let resp = ui.checkbox(
                    &mut use_linear,
                    "Use linear interpolation for color schemes",
                );
                self.use_linear_sampler.store(use_linear);
                resp
            };
        });

        SettingsUiResponse {
            response: resp.response,
        }
    }
}
