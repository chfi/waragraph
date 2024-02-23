import * as chroma from 'chroma-js';
import { Color, Scale } from 'chroma-js';


export const spectralScale: Scale<Color> = chroma.scale([
  [64, 64, 64],
  [127, 127, 127],
  [158, 1, 66],
  [213, 62, 79],
  [244, 109, 67],
  [253, 174, 97],
  [254, 224, 139],
  [255, 255, 191],
  [230, 245, 152],
  [171, 221, 164],
  [102, 194, 165],
  [50, 136, 189],
  [94, 79, 162]
]).domain([0, 12]);


export function applyColorScaleToBuffer(
  colorScale: Scale<Color>,
  dataArray: Uint32Array,
  dstColorArray: Uint32Array,
) {

  const colorBytes = new Uint8Array(dstColorArray.buffer);

  dataArray.forEach((val, i) => {
    let color = colorScale(val);
    let [r, g, b] = color.rgb();
    colorBytes[i * 4] = r;
    colorBytes[i * 4 + 1] = g;
    colorBytes[i * 4 + 2] = b;
    colorBytes[i * 4 + 3] = 255;
  });
}
