import { describe, expect, it } from 'vitest';
import { hit, gamePoint, movable, resizable, number, Widget, Rect } from './model';
const node = (id: string, kind: string, props: Record<string,string>): Widget => ({ id, kind, name: id, layers: [], warnings: [], children: [], properties: Object.entries(props).map(([key,value])=>({key,value,source:'test',local:true})) });
describe('frontend geometry adapter and editing constraints', () => {
  it('distinguishes empty values from omitted defaults', () => { const n=node('0','FRAME',{'EXTENT/WIDTH':''});expect(number(n,'EXTENT/WIDTH',100)).toBe(0);expect(number(n,'EXTENT/HEIGHT',100)).toBe(100); });
  it('uses supplied backend clip rectangles for editor selection', () => { const rect:Rect={node:node('0','LABEL',{}),x:100,y:0,w:50,h:20,parent:null,clip:[0,0,110,100]};expect(hit([rect],105,10)?.node.id).toBe('0');expect(hit([rect],120,10)).toBeUndefined(); });
  it('blocks geometry controlled by box and fill modes', () => {const rect:Rect={node:node('0','LABEL',{}),x:0,y:0,w:50,h:20,parent:node('p','BOXLAYOUT',{}),clip:[0,0,100,100]};expect(movable(rect)).toBe(false);rect.parent=null;expect(movable(rect)).toBe(true);rect.node=node('0','LABEL',{'LAYOUTMODEH':'1'});expect(resizable(rect)).toBe(false);});
  it('converts zoom to game coordinates', () => { expect(gamePoint(60,80,10,20,2)).toEqual([25,30]); });
});