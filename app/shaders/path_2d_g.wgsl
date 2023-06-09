
struct VertConfig {
  node_width: f32,
}


/// vertex shader

struct VertexOut {
  @location(0) uv: vec2f,
  @location(1) node_id: u32,
}

@group(0) @binding(0) var<uniform> transform: mat4x4f;
@group(0) @binding(1) var<uniform> config: VertConfig;

@vertex
fn vs_main(
           @builtin(vertex_index) vertex_index: u32,
           @location(0) p0: vec2f,
           @location(1) p1: vec2f,
           @location(2) node_id: u32,
) -> VertexOut {
  var result: VertexOut;
  result.node_id = node_id;

  let start = transform * vec4f(p0, 0.0, 1.0);
  let end = transform * vec4f(p1, 0.0, 1.0);


  return result;
}



/// fragment shader

struct FragmentOut {
  @location(0) color: vec4f,
  @location(1) node_id: u32,
  @location(2) uv: vec2f,
}


@fragment
fn fs_main(
           @location(0) uv: vec2f,
           @location(1) @interpolate(flat) node_id: u32,
) -> FragmentOut {
  var result: FragmentOut;

  return result;
}
