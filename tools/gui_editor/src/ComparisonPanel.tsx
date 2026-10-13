import React, { useEffect, useRef, useState } from 'react';
import { comparePixels } from './comparison';

type Picture = { name: string; image: HTMLImageElement; pixels: ImageData };
export function ComparisonPanel() {
  const [reference, setReference] = useState<Picture>(), [preview, setPreview] = useState<Picture>();
  const [mode, setMode] = useState('overlay'), [opacity, setOpacity] = useState(.5), [tolerance, setTolerance] = useState(0);
  const [error, setError] = useState(''), [metrics, setMetrics] = useState('');
  const canvas = useRef<HTMLCanvasElement>(null);
  async function load(file: File | undefined, receive: (picture: Picture) => void) {
    if (!file) return;
    const url = URL.createObjectURL(file);
    try {
      const image = new Image(); image.src = url; await image.decode();
      if (image.width * image.height > 16_777_216) throw Error('Reference image exceeds 16 megapixels.');
      const offscreen = document.createElement('canvas'); offscreen.width = image.width; offscreen.height = image.height;
      const context = offscreen.getContext('2d')!; context.drawImage(image, 0, 0);
      receive({ name: file.name, image, pixels: context.getImageData(0, 0, image.width, image.height) }); setError('');
    } catch (e) { setError(String(e)); } finally { URL.revokeObjectURL(url); }
  }
  useEffect(() => {
    setMetrics(''); setError(''); const target = canvas.current; if (!target) return;
    const first = reference ?? preview; if (!first) { target.width = target.height = 1; return; }
    target.width = first.image.width; target.height = first.image.height; const context = target.getContext('2d')!;
    if (!reference || !preview) { context.drawImage(first.image, 0, 0); return; }
    if (reference.image.width !== preview.image.width || reference.image.height !== preview.image.height) { setError('Dimensions differ. Supply matching native-resolution crops; no automatic resizing is performed.'); return; }
    const result = comparePixels(reference.pixels.data, preview.pixels.data, target.width, target.height, tolerance);
    setMetrics(`${result.changed} / ${result.total} pixels exceed tolerance · mean channel error ${result.meanAbsoluteChannelError.toFixed(3)} · maximum ${result.maximumChannelError}`);
    if (mode === 'difference') context.putImageData(new ImageData(result.pixels, target.width, target.height), 0, 0);
    else { context.drawImage(reference.image, 0, 0); context.globalAlpha = opacity; context.drawImage(preview.image, 0, 0); context.globalAlpha = 1; }
  }, [reference, preview, mode, opacity, tolerance]);
  return <section className="comparison"><p>Compare game reference and rendered output at identical pixel dimensions. Images remain in memory. This tool measures differences; it does not certify fidelity.</p>
    <label>Game reference<input type="file" accept="image/png" onChange={e => void load(e.target.files?.[0], setReference)} /></label>
    <label>Rendered output<input type="file" accept="image/png" onChange={e => void load(e.target.files?.[0], setPreview)} /></label>
    <p>{reference?.name} {reference && `${reference.image.width} × ${reference.image.height}`}<br />{preview?.name} {preview && `${preview.image.width} × ${preview.image.height}`}</p>
    <label>View<select value={mode} onChange={e => setMode(e.target.value)}><option value="overlay">Overlay</option><option value="difference">Difference</option></select></label>
    <label>Overlay opacity<input type="range" min="0" max="1" step="0.01" value={opacity} onChange={e => setOpacity(+e.target.value)} /></label>
    <label>Channel tolerance<input type="number" min="0" max="255" value={tolerance} onChange={e => setTolerance(Math.max(0, Math.min(255, Math.trunc(+e.target.value))))} /></label>
    {error && <p role="alert">{error}</p>}<p>{metrics}</p><div style={{ overflow: 'auto', maxHeight: 500 }}><canvas ref={canvas} style={{ maxWidth: 'none', imageRendering: 'pixelated' }} /></div>
  </section>;
}
