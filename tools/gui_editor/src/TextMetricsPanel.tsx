import { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
type Metrics={size:number[];physical_size:number[];selected_face:string;height:number;ascent:number;descent:number;internal_leading:number;external_leading:number};
export function NativeFontProbe({language,font,source}:{language:string;font:string;source:string}){
  const [text,setText]=useState('Anno 1404'),[metrics,setMetrics]=useState<Metrics|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
  async function measure(){setBusy(true);setError('');setMetrics(null);try{setMetrics(await invoke<Metrics>('native_font_metrics',{language,font,source,text,scale:[1,1]}))}catch(e){setError(String(e))}finally{setBusy(false)}}
  return <div><label>Single-run metric probe<input aria-label="Native metric sample" value={text} onChange={e=>setText(e.target.value)}/></label><button disabled={busy} onClick={()=>void measure()}>Measure with this source font</button>
    {error&&<p className="warning">{error}</p>}{metrics&&<p>GDI face: {metrics.selected_face}<br/>Unit-scale run: {metrics.size.join(' × ')} px<br/>Cell height {metrics.height}, ascent {metrics.ascent}, descent {metrics.descent}, internal leading {metrics.internal_leading}.</p>}
  </div>;
}
type Report={counts:Record<string,number>;rows:{text:string;delta:number[];live_size:number[];measured:Metrics;font:{Name:string}}[];diagnostics:string[]};
type FlowReport={exact:number;mismatch:number;unsupported:number;scope:string;rows:{widget:number;status:string;reason:string;predicted:{text:string;size:number[];offset:number[];line_rect:number[]}[]}[]};
export function TextFlowPanel(){
  const [language,setLanguage]=useState(''); const [report,setReport]=useState<FlowReport|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
  async function load(file:File){setBusy(true);setError('');setReport(null);try{if(file.size>32_000_000)throw Error('Capture exceeds 32MB');setReport(await invoke<FlowReport>('text_flow_compare',{snapshot:await file.text(),language:language||null}))}catch(e){setError(String(e))}finally{setBusy(false)}}
  return <details><summary>Text part placement comparison</summary><p className="muted">Import a text capture with original strings, font names and retained layout widths. Cached text/icon sizes, offsets, colors and line bounds are comparison outputs. New captures include inline parts; older captures check plain geometry only.</p><label>GUID source language<select aria-label="Text placement language" value={language} onChange={e=>{setLanguage(e.target.value);setReport(null)}}><option value="">Direct strings only</option><option value="eng">Extracted English</option><option value="ger">Extracted German</option></select></label><input type="file" accept=".json" aria-label="Text placement snapshot" disabled={busy} onChange={e=>{const file=e.target.files?.[0];if(file)void load(file)}}/>{error&&<p className="warning">{error}</p>}{report&&<><p>{report.exact} exact · {report.mismatch} mismatches · {report.unsupported} unsupported</p><p className="muted">{report.scope}</p>{report.rows.filter(r=>r.status!=='exact').slice(0,40).map((r,i)=><p className={r.status==='mismatch'?'warning':'muted'} key={i}>{r.widget}: {r.status} {r.reason}</p>)}</>}</details>;
}
export function TextMetricsPanel(){
  const [report,setReport]=useState<Report|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false);
  async function load(file:File){setBusy(true);setError('');setReport(null);try{if(file.size>32_000_000)throw Error('Capture exceeds 32MB');setReport(await invoke<Report>('text_metrics_compare',{snapshot:await file.text()}))}catch(e){setError(String(e))}finally{setBusy(false)}}
  return <details><summary>Native text metric comparison</summary><p className="muted">Import a capture-text-reference.ps1 text snapshot. Font settings are inputs; cached run sizes are comparison outputs.</p>
    <input type="file" accept=".json" aria-label="Text metric snapshot" disabled={busy} onChange={e=>{const file=e.target.files?.[0];if(file)void load(file)}}/>
    {error&&<p className="warning">{error}</p>}{report&&<><p>{report.counts.exact??0} exact · {report.counts.mismatch??0} mismatches</p>{report.diagnostics.map((d,i)=><p className="muted" key={i}>{d}</p>)}
      {report.rows.filter(r=>r.delta.some(n=>n!==0)).slice(0,40).map((r,i)=><p className="warning" key={i}>{r.font.Name}: {r.text} — live {r.live_size.join(' × ')}, GDI {r.measured.size.join(' × ')}</p>)}</>}
  </details>;
}
