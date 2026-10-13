import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { NativeFontProbe } from './TextMetricsPanel';
import { NativeGlyphPanel } from './NativeGlyphPanel';
type Candidate={source:string;sha256:string;value:string};
type Inspection={texts:Candidate[];fonts:Candidate[];diagnostics:string[]};
export function TextPanel({guid,font}:{guid:string;font:string}){
  const [language,setLanguage]=useState('eng'),[query,setQuery]=useState(guid),[fontQuery,setFontQuery]=useState(font),[data,setData]=useState<Inspection|null>(null),[error,setError]=useState('');
  useEffect(()=>{setQuery(guid);setFontQuery(font)},[guid,font]);
  useEffect(()=>{let stale=false;setData(null);setError('');const number=Number(query);
    if(query!==''&&(!Number.isInteger(number)||number<-2147483648||number>2147483647)){setError('Enter an integer text GUID');return}
    void invoke<Inspection>('localisation_inspect',{language,guid:query===''?-1:number,font:fontQuery}).then(result=>{if(!stale)setData(result)}).catch(e=>{if(!stale)setError(String(e))});return()=>{stale=true};
  },[language,query,fontQuery]);
  return <details><summary>Extracted text and font resources</summary><p className="muted">Raw source candidates. Supported English LABEL captions render on the canvas; runtime text and full pixel fidelity remain unverified.</p>
    <label>Language<select value={language} onChange={e=>setLanguage(e.target.value)}><option value="eng">English</option><option value="ger">German</option></select></label>
    <label>Text GUID<input aria-label="Localization text GUID" value={query} onChange={e=>setQuery(e.target.value)}/></label>
    <label>Named font<input aria-label="Localization named font" value={fontQuery} onChange={e=>setFontQuery(e.target.value)}/></label>
    {error&&<p className="warning">{error}</p>}{data?.diagnostics.map((d,i)=><p className="muted" key={i}>{d}</p>)}
    {data?.texts.map((c,i)=><div key={'t'+i}><small>{c.source}</small><pre>{c.value}</pre></div>)}
    {data?.fonts.map((c,i)=><div key={'f'+i}><small>{c.source}</small><pre>{c.value}</pre><NativeFontProbe language={language} font={fontQuery} source={c.source}/><NativeGlyphPanel language={language} font={fontQuery} source={c.source}/></div>)}
  </details>;
}
