import React,{useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
import {ButtonSelection} from './ButtonStatePanel';
type Report={captured_utc:string;counts:Record<string,number>;rows:{widget:number;selection:ButtonSelection;actual_index:number;status:string}[];diagnostics:string[]};
export function ButtonStateComparisonPanel(){
  const[report,setReport]=useState<Report>(),[error,setError]=useState('');
  async function load(file:File|undefined){if(!file)return;setReport(undefined);setError('');try{if(file.size>32_000_000)throw Error('Snapshot exceeds 32 MB');setReport(await invoke<Report>('button_states_compare',{snapshot:await file.text()}))}catch(e){setError(String(e))}}
  return <details><summary>Live button-state comparison</summary><p className="muted">Load schema-1 JSON from capture-button-reference.ps1. Existing visible-chain background indices are comparison outputs; captured states and layout names supply the inputs.</p><input type="file" accept="application/json,.json" aria-label="Button state snapshot" onChange={e=>void load(e.target.files?.[0])}/>{error&&<p role="alert">{error}</p>}{report&&<><p>{Object.entries(report.counts).map(([k,v])=>`${k}: ${v}`).join(' · ')} · {report.captured_utc}</p>{report.diagnostics.map(d=><p className="muted" key={d}>{d}</p>)}{report.rows.filter(r=>r.status==='mismatch').map(r=><p className="warning" key={r.widget}>{r.widget.toString(16)}: {r.selection.requested} → {r.selection.selected} index {r.selection.index}; live {r.actual_index}</p>)}</>}</details>;
}
