use std::{collections::HashMap, sync::Arc};
use waragraph_core::graph::{PathId, PathIndex};

use crate::{app::SharedState, color::ColorMap};

use super::render::VizModeConfig;

pub(super) fn create_path_name_hash_colors<'a>(
    paths: impl Iterator<Item = (PathId, &'a str)>,
) -> Vec<[f32; 4]> {
    let mut paths = paths.collect::<Vec<_>>();
    paths.sort_by_key(|(p, _)| *p);

    let mut colors = Vec::with_capacity(paths.len());

    for (_, path_name) in paths {
        let [r, g, b] = crate::color::util::path_name_hash_color(path_name);
        colors.push([r, g, b, 1.]);
    }

    colors
}

pub(super) fn init_path_name_hash_viz_mode(
    state: &raving_wgpu::State,
    shared: &SharedState,
    viz_samplers: &mut HashMap<
        String,
        Arc<dyn super::sampler::Sampler + 'static>,
    >,

    viz_mode_config: &mut HashMap<String, VizModeConfig>,
) {
    // create sampler
    let path_count = shared.graph.path_names.len();
    let sampler = super::sampler::PathNodeSetSampler::new(
        shared.graph.clone(),
        move |path, _| {
            let dx = 1.0 / path_count as f32;
            let x = path.ix() as f32 / path_count as f32;
            0.5 * dx + x
        },
    );

    viz_samplers.insert("path_name".to_string(), Arc::new(sampler) as Arc<_>);

    // create color buffer
    let path_names = shared
        .graph
        .path_names
        .iter()
        .map(|(p, n)| (*p, n.as_str()));
    let color_vec = create_path_name_hash_colors(path_names);

    // create color scheme & upload texture
    let color_scheme = {
        let mut colors = shared.colors.blocking_write();
        let id = colors.add_color_scheme("path_name_hash", color_vec);
        colors.create_color_scheme_texture(state, "path_name_hash");
        id
    };

    let path_name = VizModeConfig {
        name: "path_name".to_string(),
        data_key: "path_name".to_string(),
        color_scheme,
        default_color_map: ColorMap {
            value_range: [0.0, 1.0],
            color_range: [0.0, 1.0],
        },
    };

    viz_mode_config.insert("path_name".to_string(), path_name);

    shared
        .data_color_schemes
        .blocking_write()
        .insert("path_name".into(), color_scheme);
}
