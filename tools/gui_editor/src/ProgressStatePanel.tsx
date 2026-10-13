import React from 'react';
export type ProgressInput={current?:number;maximum?:number};
export function ProgressStatePanel({value,current,maximum,onChange}:{value:ProgressInput;current:string;maximum:string;onChange:(value:ProgressInput)=>void}){
  return <details open><summary>Progress preview value</summary>{(['current','maximum'] as const).map(key=><label key={key}>{key}<input type="number" min={-2147483648} max={2147483647} step={1} value={value[key]??''} placeholder={key==='current'?current||'0':maximum||'10'} onChange={e=>{const text=e.target.value,n=Number(text);if(text===''||(Number.isInteger(n)&&n>=-2147483648&&n<=2147483647))onChange({...value,[key]:text===''?undefined:n})}}/></label>)}<p className="muted">Separate scenario values. Blank uses source/constructor values. Steady unit-scale background, bar and overlay layers; animations and pixels remain unverified.</p></details>;
}
