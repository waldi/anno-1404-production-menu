import React from 'react';
import { Widget } from './model';
export type LayoutScenario={passes:number;island_info:{parent_viewport:[number,number];wares_right:number}|null};
export type LayoutPass={index:number;root_raw:number[];identical_to_previous:boolean};
export function LayoutScenarioPanel({root,width,height,value,onChange,passes}:{root:Widget;width:number;height:number;value:LayoutScenario;onChange:(value:LayoutScenario)=>void;passes:LayoutPass[]}){
  const island=root.kind==='FRAME'&&root.name==='INFOBAR_ISLAND_INFO';
  const last=passes.at(-1);
  return <details open><summary>Layout update scenario</summary>
    <label>Update passes<input aria-label="Layout update passes" type="number" min={1} max={32} step={1} value={value.passes} onChange={e=>{const n=Number(e.target.value);if(Number.isInteger(n)&&n>=1&&n<=32)onChange({...value,passes:n})}}/></label>
    {island&&<><label><input type="checkbox" checked={!!value.island_info} onChange={e=>onChange({...value,island_info:e.target.checked?{parent_viewport:[width,height],wares_right:0}:null})}/> Position island panel in a full-viewport parent</label>
      {value.island_info&&<><label>External wares panel right<input aria-label="External wares panel right" type="number" min={0} max={4096} step={1} value={value.island_info.wares_right} onChange={e=>{const n=Number(e.target.value);if(value.island_info&&Number.isInteger(n)&&n>=0&&n<=4096)onChange({...value,island_info:{...value.island_info,wares_right:n}})}}/></label><p className="muted">Parent scenario: {value.island_info.parent_viewport.join(' × ')}. Re-enable positioning to bind a changed viewport.</p></>}
    </>}
    <p className="muted">Starts from source geometry and retains each pass in native action order. Pass count is explicit; transition timing, slot contents and animation are unverified. These inputs stay separate from XML.</p>
    {last&&<p>Pass {last.index}: root [{last.root_raw.join(', ')}] · {last.identical_to_previous?'geometry unchanged from preceding pass':'geometry changed'}</p>}
    {!!passes.length&&<details><summary>Pass trace</summary><pre>{passes.map(p=>`${p.index}: [${p.root_raw.join(', ')}]${p.identical_to_previous?' unchanged':''}`).join('\n')}</pre></details>}
  </details>;
}
