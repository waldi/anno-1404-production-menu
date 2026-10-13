export type Property = { key: string; value: string; source: string; local: boolean };
export type Widget = { id: string; kind: string; name: string; properties: Property[]; layers: Property[][]; warnings: string[]; children: Widget[] };
export type View = { key: string; xml: string; original: string; revision: number; dirty: boolean; can_undo: boolean; can_redo: boolean; tree: Widget | null; states: string[]; preview: string | null; saved_to: string | null };
export type Catalog = { files: string[]; counts: Record<string, number>; repo: string; errors: string[] };
export type Rect = { node: Widget; x: number; y: number; w: number; h: number; parent: Widget | null; clip: [number, number, number, number] };
export const prop = (n: Widget, key: string) => n.properties.find(p => p.key === key)?.value ?? '';
export const number = (n: Widget, key: string, fallback = 0) => { const p = n.properties.find(p => p.key === key); return !p ? fallback : Number.isFinite(Number(p.value)) ? Number(p.value) : fallback; };
export function movable(r: Rect) {
  return r.parent?.kind !== 'BOXLAYOUT' && !['LAYOUTMODEH', 'LAYOUTMODEV'].some(k => number(r.node, k) !== 0);
}
export function resizable(r: Rect) { return !['LAYOUTMODEH', 'LAYOUTMODEV'].some(k => number(r.node, k) === 1); }
export const editable = (key: string) => ['PATHID', 'ACCESSID', 'TEMPLATE_REF_ID', 'BACKGROUND', 'HIDDEN', 'TRANSPARENT', 'ZORDER', 'ALPHA', 'ICONGUID', 'ICONSTRETCH', 'EXTENT/LEFT', 'EXTENT/TOP', 'EXTENT/WIDTH', 'EXTENT/HEIGHT', 'XSPACE', 'YSPACE', 'LINE_ORDER', 'LINECOLUMNCOUNT', 'TEXTSETTING/TEXTGUID', 'TEXTSETTING/FONT'].includes(key);
export function hit(rects: Rect[], x: number, y: number) {
  return [...rects].reverse().find(r => r.w > 0 && r.h > 0 && x >= r.x && y >= r.y && x <= r.x + r.w && y <= r.y + r.h && x >= r.clip[0] && y >= r.clip[1] && x <= r.clip[2] && y <= r.clip[3]);
}
export function gamePoint(x: number, y: number, left: number, top: number, zoom: number): [number, number] { return [(x - left) / zoom, (y - top) / zoom]; }
