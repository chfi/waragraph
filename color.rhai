export let bg_light = #{ r: 0.95, g: 0.95, b: 0.95 };
export let bg_dark = #{ r: 0.05, g: 0.05, b: 0.05 };


let spectral = allocate_vec4_buffer("gradient-colorbrewer-spectral", 256);
fill_vec4_buffer(spectral,
                 [rgba(255, 255, 255),
                  rgba(196, 196, 196),
                  rgba(128, 128, 128),
                  rgba(158,1,66),
                  rgba(213,62,79),
                  rgba(244,109,67),
                  rgba(253,174,97),
                  rgba(254,224,139),
                  rgba(255,255,191),
                  rgba(230,245,152),
                  rgba(171,221,164),
                  rgba(102,194,165),
                  rgba(50,136,189),
                  rgba(94,79,162)]);


let category10 = allocate_vec4_buffer("gradient-category10", 11);
{
  let colors = [];
  colors.push(rgba(1.0, 1.0, 1.0));
  colors.push(rgba(0x1f, 0x77, 0xb4));
  colors.push(rgba(0xff, 0x7f, 0x0e));
  colors.push(rgba(0x2c, 0xa0, 0x2c));
  colors.push(rgba(0xd6, 0x27, 0x28));
  colors.push(rgba(0x94, 0x67, 0xbd));
  colors.push(rgba(0x8c, 0x56, 0x4b));
  colors.push(rgba(0xe3, 0x77, 0xc2));
  colors.push(rgba(0x7f, 0x7f, 0x7f));
  colors.push(rgba(0xbc, 0xbd, 0x22));
  colors.push(rgba(0x17, 0xbe, 0xcf));

  fill_vec4_buffer(category10, colors);
}

let grayscale = allocate_vec4_buffer("gradient-grayscale", 256);
let grayscale_inv = allocate_vec4_buffer("gradient-grayscale-invert", 256);
{
  let colors = [];
    // colors.push(rgba(64, 0, 0));
    // colors.push(rgba(255, 255, 255));
    colors.push(rgba(255, 255, 255));
  for i in 1..256 {
  // for i in 0..256 {
    colors.push(rgba(i, i, i));
  }
  fill_vec4_buffer(grayscale, colors);

  colors.reverse();
  fill_vec4_buffer(grayscale_inv, colors);
}



let gui_palette = allocate_vec4_buffer("gui-palette", 32);
fill_vec4_buffer(gui_palette,
                 [rgba(0, 0, 0),
                  rgba(32, 32, 32),
                  rgba(64, 64, 64),
                  rgba(96, 96, 96),
                  rgba(128, 128, 128),
                  rgba(160, 160, 160),
                  rgba(192, 192, 192),
                  rgba(224, 224, 224),
                  rgba(255, 255, 255),
                  rgba(32, 0, 0),
                  rgba(64, 0, 0),
                  rgba(128, 0, 0),
                  rgba(160, 0, 0),
                  rgba(192, 0, 0),
                  rgba(0, 32, 0),
                  rgba(0, 64, 0),
                  rgba(0, 128, 0),
                  rgba(0, 160, 0),
                  rgba(0, 192, 0),
                  rgba(0, 0, 32),
                  rgba(0, 0, 64),
                  rgba(0, 0, 128),
                  rgba(0, 0, 160),
                  rgba(0, 0, 192),
                 ]);
