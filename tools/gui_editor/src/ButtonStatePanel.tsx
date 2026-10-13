import React from 'react';
export type ButtonInput={hovered:boolean;pressed:boolean;checked:boolean;enabled?:boolean};
export type ButtonSelection={requested:string;selected:string;index:number;fallback:boolean};
export function ButtonStatePanel({value,selection,onChange}:{value:ButtonInput;selection?:ButtonSelection;onChange:(value:ButtonInput)=>void}){
  return <details open><summary>Button preview state</summary>
    {(['hovered','pressed','checked'] as const).map(key=><label key={key}><input type="checkbox" checked={value[key]} onChange={e=>onChange({...value,[key]:e.target.checked})}/>{key}</label>)}
    <label>Enabled<select value={value.enabled===undefined?'source':value.enabled?'yes':'no'} onChange={e=>onChange({...value,enabled:e.target.value==='source'?undefined:e.target.value==='yes'})}><option value="source">Source and parent chain</option><option value="yes">Enabled locally</option><option value="no">Disabled locally</option></select></label>
    {selection&&<p>{selection.requested} → {selection.selected} (index {selection.index}){selection.fallback?' · missing variant uses first layout':''}</p>}
    <p className="muted">Scenario inputs only. Runtime forced states, focus overlay, text color and effects still require verification.</p>
  </details>;
}
