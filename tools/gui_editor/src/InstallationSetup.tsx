import React, {useEffect, useState} from 'react';
import {invoke} from '@tauri-apps/api/core';

export type InstallationStatus = {selection:{game:string;explorer:string};ready:boolean;preparing:boolean;message:string};
export function InstallationSetup({initial,onReady,onClose}:{initial:InstallationStatus;onReady:(s:InstallationStatus)=>void;onClose?:()=>void}) {
  const [game,setGame]=useState(initial.selection.game),[explorer,setExplorer]=useState(initial.selection.explorer);
  const [busy,setBusy]=useState(initial.preparing),[message,setMessage]=useState(initial.message),[error,setError]=useState(''),[picking,setPicking]=useState(false);
  useEffect(()=>{if(!busy)return;let alive=true;const timer=setInterval(()=>{void invoke<InstallationStatus>('installation_status').then(s=>{if(alive)setMessage(s.message)}).catch(()=>{})},500);return()=>{alive=false;clearInterval(timer)}},[busy]);
  async function pick(set:(s:string)=>void){setPicking(true);setError('');try{const path=await invoke<string|null>('choose_directory');if(path)set(path)}catch(e){setError(String(e))}finally{setPicking(false)}}
  async function prepare(){setBusy(true);setError('');setMessage('Validating folders…');try{onReady(await invoke<InstallationStatus>('prepare_installation',{game,explorer}))}catch(e){setError(String(e))}finally{setBusy(false)}}
  return <div className="setup-overlay"><section className="installation-setup" aria-label="Game installation setup">
    <h1>{initial.ready?'Game installation settings':'Welcome to Anno GUI Studio'}</h1>
    <p>Select your Venice or History Edition installation and unpacked RDA Explorer folder. GUI Studio will prepare the resources it needs automatically.</p>
    <label>1. Anno 1404 game directory<div className="setup-path"><input aria-label="Game directory" value={game} disabled={busy||picking} onChange={e=>setGame(e.target.value)} placeholder="Folder containing Addon.exe or Anno1404Addon.exe"/><button disabled={busy||picking} onClick={()=>void pick(setGame)}>Browse…</button></div></label>
    <label>2. RDA Explorer directory<div className="setup-path"><input aria-label="RDA Explorer directory" value={explorer} disabled={busy||picking} onChange={e=>setExplorer(e.target.value)} placeholder="Folder containing RDAExplorer.dll"/><button disabled={busy||picking} onClick={()=>void pick(setExplorer)}>Browse…</button></div></label>
    <p className="muted">The game directory remains read-only. Prepared resources and saved edits are stored separately on this computer. English and German resources are supported initially.</p>
    {message&&<p role="status" aria-live="polite">{message}</p>}{busy&&<progress aria-label="Preparing game resources"/>}
    {error&&<p role="alert" className="warning">{error}</p>}
    <div className="setup-actions">{onClose&&<button disabled={busy||picking} onClick={onClose}>Close</button>}{busy?<button onClick={()=>void invoke('cancel_preparation')}>Cancel preparation</button>:<button className="primary" disabled={!game.trim()||!explorer.trim()||picking} onClick={()=>void prepare()}>Prepare and open editor</button>}</div>
  </section></div>
}
