/** Native-resolution RGBA comparison; deliberately rejects scaling/cropping. */
export function comparePixels(a: Uint8ClampedArray, b: Uint8ClampedArray, width: number, height: number, tolerance = 0) {
  if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0 || a.length !== width * height * 4 || b.length !== a.length) throw Error('Reference and preview must have identical native pixel dimensions.');
  if (!Number.isInteger(tolerance) || tolerance < 0 || tolerance > 255) throw Error('Tolerance must be an integer from 0 to 255.');
  const pixels = new Uint8ClampedArray(a.length); let changed = 0, absolute = 0, maximum = 0;
  for (let i = 0; i < a.length; i += 4) {
    let difference = 0;
    for (let channel = 0; channel < 4; channel++) {
      const d = Math.abs(a[i + channel] - b[i + channel]); absolute += d; maximum = Math.max(maximum, d); difference = Math.max(difference, d);
    }
    if (difference > tolerance) changed++;
    pixels[i] = difference; pixels[i + 1] = 0; pixels[i + 2] = difference; pixels[i + 3] = 255;
  }
  return { pixels, changed, total: width * height, meanAbsoluteChannelError: absolute / a.length, maximumChannelError: maximum };
}
