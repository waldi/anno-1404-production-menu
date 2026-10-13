import React from 'react';
export type TextRefreshInput={client_width:number;fit_wrapped_width:boolean};
export function TextRefreshPanel({value,onChange}:{value:TextRefreshInput|undefined;onChange:(value:TextRefreshInput|undefined)=>void}){
  return <details open><summary>Text refresh scenario</summary>
    <label>Pre-refresh client width<input aria-label="Pre-refresh client width" type="number" min={1} max={8192} step={1} value={value?.client_width??''} onChange={e=>{const text=e.target.value,n=Number(text);if(text==='')onChange(undefined);else if(Number.isInteger(n)&&n>=1&&n<=8192)onChange({client_width:n,fit_wrapped_width:value?.fit_wrapped_width??false})}}/></label>
    <label><input type="checkbox" disabled={!value} checked={value?.fit_wrapped_width??false} onChange={e=>{if(value)onChange({...value,fit_wrapped_width:e.target.checked})}}/> Fit wrapped width to text</label>
    <p className="muted">Separate preview input. Wrapping keeps this client width unless fit is selected. Blank leaves constrained owner resizing unresolved. The initial owner uses authored position and height with this client width. Background borders are added after measurement; parent layout may then move or resize the widget. Refresh history and full dialog pixels remain unverified.</p>
  </details>;
}
