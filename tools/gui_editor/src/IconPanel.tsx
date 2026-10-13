import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
type Icon = { guid:number; variation:number; file_id:number; index:number; source:number[]; texture:string; texture_source:string; png:string; diagnostics:string[] };
export function IconPanel({guid,archiveCandidate}:{guid:number;archiveCandidate:string}) {
  const [variation,setVariation]=useState(0),[icon,setIcon]=useState<Icon>(),[error,setError]=useState('');
  useEffect(()=>{let stale=false;setIcon(undefined);setError('');void invoke<Icon>('icon_asset',{guid,variation,archiveCandidate:archiveCandidate||null}).then(i=>{if(!stale)setIcon(i)}).catch(e=>{if(!stale)setError(String(e))});return()=>{stale=true}},[guid,variation,archiveCandidate]);
  return <details open><summary>Icon atlas crop · GUID {guid}</summary><label>Explicit variation <input type="number" min="0" value={variation} onChange={e=>setVariation(Math.max(0,Math.trunc(+e.target.value)))}/></label>{error&&<p className="warning">{error}</p>}{icon&&<><img src={icon.png} alt={'Icon GUID '+guid} style={{width:92,height:92,imageRendering:'pixelated'}}/><p>File {icon.file_id} · index {icon.index} · rectangle {icon.source.join(', ')}</p><p className="muted">{icon.texture}<br/>{icon.texture_source}</p><p className="warning">{icon.diagnostics.join(' ')}</p></>}</details>;
}
