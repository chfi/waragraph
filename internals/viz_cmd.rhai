set_name("Visualization");
set_desc("Visualization modes and options");

/// Set visualization mode
///
/// @viz_mode: VizMode
fn set_visualization_mode(viz_mode) {
  let cfg = viewer::cfg;
  let props = viewer::props;
  let slot_fns = slot::slot_fn_names();

  cfg.update("viz.slot_function", || {
    props.set("viz.previous", this);
    this = viz_mode;
  })
}
