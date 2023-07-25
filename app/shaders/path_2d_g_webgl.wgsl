struct VertConfig {
  node_width: f32,
  pad0: f32,
  pad1: f32,
  pad2: f32,
}


/// vertex shader

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
  @location(1) node_id: u32,
  @location(2) node_data: f32,
}

@group(0) @binding(0) var<uniform> projection: mat4x4f;
@group(0) @binding(1) var<uniform> config: VertConfig;

// @vertex
/*
fn vs_main_old(
           @builtin(vertex_index) vertex_index: u32,
           @location(0) p0: vec2f,
           @location(1) p1: vec2f,
           @location(2) node_id: u32,
) -> VertexOut {
  var result: VertexOut;
  result.node_id = node_id;

  let start = projection * vec4f(p0, 0.0, 1.0);
  let end = projection * vec4f(p1, 0.0, 1.0);


  return result;
}
*/

@vertex
fn vs_main(
           @builtin(vertex_index) vertex_index: u32,
           @location(0) p0: vec2f,
           @location(1) p1: vec2f,
           @location(2) node_id: u32,
           @location(3) node_data: f32,
) -> VertexOut {
  var result: VertexOut;
  result.node_id = node_id;

  let i = vertex_index % 6u;

  var pos: vec2f;

  switch i {
      case 0u: {
        pos = vec2(0.0, -0.5);
      }
      case 1u: {
        pos = vec2(1.0, -0.5);
      }
      case 2u: {
        pos = vec2(1.0, 0.5);
      }
      case 3u: {
        pos = vec2(0.0, -0.5);
      }
      case 4u: {
        pos = vec2(1.0, 0.5);
      }
      // case 5u: {
      //   pos = vec2(0.0, 0.5);
      // }
      default: {
        pos = vec2(0.0, 0.5);
      }
  }

  let x_basis = p1 - p0;
  let y_basis = normalize(vec2(-x_basis.y, x_basis.x));

  let point = p0 + x_basis * pos.x + y_basis * config.node_width * pos.y;

  result.position = projection * vec4(point, 0.0, 1.0);
  result.node_data = node_data;

  return result;
}


/// fragment shader

struct FragmentOut {
  @location(0) color: vec4f,
  // @location(1) node_id: u32,
  // @location(2) uv: vec2f,
}

struct ColorMap {
 min_val: f32,
 max_val: f32,
 min_color: f32,
 max_color: f32,
}

struct DataConfig {
 page_size: u32,
 // pad_: u32,
 // offset: u32,
 // max: u32,
}

// @group(0) @binding(0) var<uniform> projection: mat4x4f;
// @group(0) @binding(1) var<uniform> config: VertConfig;

// @group(1) @binding(0) var<storage, read> u_data: array<f32>;
// @group(1) @binding(0) var<storage, read> colors: array<vec4f>;

@group(1) @binding(0) var<uniform> u_data_config: DataConfig;

@group(1) @binding(1) var t_sampler: sampler;
@group(1) @binding(2) var t_colors: texture_1d<f32>;

@group(1) @binding(3) var<uniform> u_color_map: ColorMap;


@fragment
fn fs_main(
           @location(0) uv: vec2f,
           @location(1) @interpolate(flat) node_id: u32,
           @location(2) node_data: f32,
) -> FragmentOut {
  var result: FragmentOut;

  // let index = (u_data_config.offset + node_id).clamp(0, u_data_config.max);
  // let index = node_id % u_data_config.page_size;

  // let v = u_data[index];
  let v_n = node_data;
  // let v_n = 0.5;
  // let v = u_data[node_id];

  // let v_n = (v - u_color_map.min_val) / (u_color_map.max_val - u_color_map.min_val);
  let c_n = mix(u_color_map.min_color, u_color_map.max_color, v_n);

  let color = textureSample(t_colors, t_sampler, c_n);

  // let color = u_colors[node_id];
  result.color = color;

  // result.node_id = node_id;
  // result.uv = uv;

  return result;
}
