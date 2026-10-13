import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { View } from './model';
import { ButtonInput } from './ButtonStatePanel';
import { TextRefreshInput } from './TextRefreshPanel';
type Row={path:string;id:string;status:string;raw_delta:number[]|null;absolute_delta:number[]|null;diagnostics:string[]};
type Report={captured_utc:string;binary_sha256:string;viewport:number[];counts:Record<string,number>;rows:Row[];diagnostics:string[]};
export function GeometryComparisonPanel({view,width,height,hidden,cards,visibility,buttons,runtime,onVisibility}:{view:View;width:number;height:number;hidden:boolean;cards:Record<string,number>;visibility:Record<string,boolean>;buttons:Record<string,ButtonInput>;runtime:{current_city_name:string|null;text_refresh:Record<string,TextRefreshInput>};onVisibility:(values:Record<string,boolean>)=>void}){
  const [snapshot,setSnapshot]=useState(''),[name,setName]=useState(''),[report,setReport]=useState<Report>(),[error,setError]=useState(''),[filter,setFilter]=useState('mismatch');
  useEffect(()=>{let stale=false;setReport(undefined);setError('');if(snapshot)invoke<Report>('geometry_compare',{snapshot,revision:view.revision,width,height,showHidden:hidden,cards,visibility,buttons,language:"eng",runtime}).then(r=>{if(!stale)setReport(r)}).catch(e=>{if(!stale)setError(String(e))});return()=>{stale=true}},[snapshot,view.revision,width,height,hidden,cards,visibility,buttons,runtime]);
  async function applyVisibility(){try{const scenario=await invoke<{visibility:Record<string,boolean>}>('geometry_scenario',{snapshot,revision:view.revision,width,height});onVisibility(scenario.visibility)}catch(e){setError(String(e))}}
  async function load(file:File|undefined){if(!file)return;try{if(file.size>32_000_000)throw Error('Snapshot exceeds 32 MB');setSnapshot(await file.text());setName(file.name)}catch(e){setError(String(e))}}
  return <section className="geometry-comparison"><h3>Live widget geometry</h3><p className="muted">Load a schema-2 snapshot from capture-reference.ps1. Matching uses the dialog name and child paths. GUI viewport must match exactly; hidden or offscreen stored rectangles are not treated as reliable absolute references.</p>
    <label>Widget snapshot<input aria-label="Widget snapshot" type="file" accept="application/json,.json" onChange={e=>void load(e.target.files?.[0])}/></label><p>{name}</p>
    <button disabled={!snapshot} onClick={()=>void applyVisibility()}>Use captured visibility</button><button disabled={!Object.keys(visibility).length} onClick={()=>onVisibility({})}>Use source visibility</button><p className="muted">{Object.keys(visibility).length} scenario overrides. Captured rectangles are never used as layout inputs; scenario state is separate from XML.</p>
    {error&&<p role="alert">{error}</p>}{report&&<><p>{Object.entries(report.counts).map(([key,count])=>`${key}: ${count}`).join(' · ')}</p><p className="muted">{report.captured_utc} · GUI {report.viewport.join(' × ')}</p>{report.diagnostics.map(d=><p className="warning" key={d}>{d}</p>)}
    <label>Results<select value={filter} onChange={e=>setFilter(e.target.value)}><option value="all">All</option>{Object.keys(report.counts).map(k=><option key={k}>{k}</option>)}</select></label>
    <div style={{maxHeight:400,overflow:'auto'}}>{report.rows.filter(r=>filter==='all'||r.status===filter).map(r=><details key={r.id}><summary>{r.path} · {r.status}</summary><p>Raw extent delta: {r.raw_delta?.join(', ')??'unavailable'}<br/>Absolute delta: {r.absolute_delta?.join(', ')??'omitted'}</p>{r.diagnostics.map((d,i)=><p key={i}>{d}</p>)}</details>)}</div></>}
  </section>;
}
