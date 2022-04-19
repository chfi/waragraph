export let bg_light = #{ r: 0.95, g: 0.95, b: 0.95 };
export let bg_dark = #{ r: 0.05, g: 0.05, b: 0.05 };


let buffer = allocate_vec4_buffer("gradient-colorbrewer-spectral", 256);
fill_vec4_buffer(buffer,
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



let buffer = allocate_vec4_buffer("gui-palette", 32);
fill_vec4_buffer(buffer,
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
