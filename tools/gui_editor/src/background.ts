export type Tile = { source: [number, number, number, number]; destination: [number, number, number, number]; flags: number; texture: string; repeat: [number, number] };
export type Background = { state: string; client_offsets: [number, number, number, number]; tiles: Tile[]; textures: Record<string, string>; sources?:Record<string,string>; diagnostics: string[] };
export function drawBackground(ctx: CanvasRenderingContext2D, data: Background, images: Record<string, HTMLImageElement>, left = 0, top = 0) {
  ctx.save(); ctx.translate(left, top);
  for (const t of data.tiles) {
    const [sx, sy, sw, sh] = t.source, [x, y, w, h] = t.destination;
    if (t.flags & 32 || sw <= 0 || sh <= 0 || w <= 0 || h <= 0) continue;
    const image = images[t.texture]; if (!image) continue;
    const stretch = !!(t.flags & 0x44), repeat = !stretch && !!(t.flags & 3);
    const bw = stretch ? w : sw * (t.repeat[0] + 1), bh = stretch ? h : sh * (t.repeat[1] + 1);
    ctx.save(); ctx.beginPath(); ctx.rect(x, y, w, h); ctx.clip();
    for (let dy = y; dy < y + h; dy += repeat ? bh : h) {
      for (let dx = x; dx < x + w; dx += repeat ? bw : w) {
        ctx.save(); ctx.translate(dx + (t.flags & 16 ? bw : 0), dy + (t.flags & 8 ? bh : 0));
        ctx.scale(t.flags & 16 ? -1 : 1, t.flags & 8 ? -1 : 1); ctx.drawImage(image, sx, sy, sw, sh, 0, 0, bw, bh); ctx.restore();
      }
    }
    ctx.restore();
  }
  ctx.restore();
}
export async function backgroundImages(data: Background) {
  const images: Record<string, HTMLImageElement> = {};
  await Promise.all(Object.entries(data.textures).map(async ([path, url]) => { const image = new Image(); image.src = url; await image.decode(); images[path] = image; }));
  return images;
}
