struct DataBuffer {
    total_size: u32,
    row_size: u32,
    values: array<f32>,
}

struct ColorMap {
    min_val: f32,
    max_val: f32,
    min_color: f32,
    max_color: f32,
}

struct SlotUniform {
    ab: vec2f,
    bin_count: u32,
}


struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
    @location(1) slot_id: u32,
}


/// vertex shader

@group(0) @binding(0) var<uniform> dims: vec2f;

@vertex
fn vs_main(
           @builtin(vertex_index) vertex_index: u32,
           @location(0) position: vec2f,
           @location(1) size: vec2f,
           @location(2) slot_id: u32,
) -> VertexOut {
    var result: VertexOut;

    var origin: vec2f = (2.0 * position / dims) - vec2(1);
    origin.y *= -1.0;

    let size = (2.0 * size) / dims;

    let i = vertex_index % 6;

    if (i == 0) {
      result.position = vec4(origin, 0.0, 1.0);
      result.uv = vec2(0);
    } else if (i == 1 || i == 4) {
      result.position = vec4(origin + vec2(size.x, 0), 0.0, 1.0);
      result.uv = vec2(1.0, 0.0);
    } else if (i == 2 || i == 3) {
      result.position = vec4(origin + vec2(0, size.y), 0.0, 1.0);
      result.uv = vec2(0.0, 1.0);
    } else {
      result.position = vec4(origin + size, 0.0, 1.0);
      result.uv = vec2(1.0, 1.0);
    }

    result.slot_id = slot_id;

    return result;
}

/// fragment shader

@group(1) @binding(0) var<storage, read> u_data: DataBuffer;

@group(1) @binding(1) var t_sampler: sampler;
@group(1) @binding(2) var t_colors: texture_1d<f32>;

@group(1) @binding(3) var<uniform> u_color_map: ColorMap;

@group(1) @binding(4) var<storage, read> u_slots: array<SlotUniform>;


@fragment
fn fs_main(
           @location(0) uv: vec2f,
           @location(1) @interpolate(flat) slot_id: u32,
) -> @location(0) vec4f {

  let row_offset = slot_id * u_data.row_size;
  let t = uv.x;

  let ab = u_slots[slot_id].ab;
  let t_ = ab.x * t + ab.y;

  let c_t = clamp(t_, 0.0, 1.0);

  let bin_count = u_slots[slot_id].bin_count;
  var data_ix = u32(round(c_t * f32(bin_count)));
  data_ix = clamp(data_ix, 0, bin_count - 1);

  let v = u_data.values[row_offset + data_ix];

  let v_n = (v - u_color_map.min_val) / (u_color_map.max_val - u_color_map.min_val);

  let c_n = mix(u_color_map.min_color, u_color_map.max_color, v_n);

  let sampled = textureSample(t_colors, t_sampler, c_n);


  // NB: wgsl doesn't really do infinities, plus it's a bit weird to
  // use infinity to signal "gaps", so this should be changed to
  // something like a specific (NaN) bit pattern
  let color = select(vec4f(1), sampled, v < 1000000.0);

  return color;
}
