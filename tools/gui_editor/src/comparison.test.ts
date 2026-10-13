import { expect, test } from 'vitest';
import { comparePixels } from './comparison';
test('compares all channels without hiding alpha errors', () => {
  const a = new Uint8ClampedArray([1, 2, 3, 255, 10, 20, 30, 255]);
  const b = new Uint8ClampedArray([1, 2, 3, 254, 14, 20, 30, 255]);
  const result = comparePixels(a, b, 2, 1);
  expect(result.changed).toBe(2); expect(result.maximumChannelError).toBe(4);
  expect(result.meanAbsoluteChannelError).toBe(5 / 8);
  expect(comparePixels(a, b, 2, 1, 1).changed).toBe(1);
});
test('rejects invalid dimensions and tolerance; identical images have zero error', () => {
  const a = new Uint8ClampedArray(4);
  expect(comparePixels(a, a, 1, 1).changed).toBe(0);
  expect(() => comparePixels(a, a, 2, 1)).toThrow();
  expect(() => comparePixels(a, a, 1, 1, -1)).toThrow();
});
