
struct VertConfig {
  node_width: f32,
}


/// vertex shader

struct VertexOut {
  @builtin(position) position: vec4f,
  @location(0) uv: vec2f,
  @location(1) node_id: u32,
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
