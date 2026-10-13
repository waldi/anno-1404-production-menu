import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Background, backgroundImages, drawBackground } from './background';

type Preview = {png:string;state:string};
// Cache belongs to this gallery/installation and artwork selection. Limit atlas
// decoding to three requests at a time while retaining lazy visible loading.
function createLoader(archiveCandidate:string) {
  const previews=new Map<string,Promise<Preview>>(),queue:(()=>void)[]=[];
  let active=0;
  function drain(){while(active<3&&queue.length){active++;queue.shift()!()}}
  return (path:string)=>{
    let request=previews.get(path);
    if(!request){
      request=new Promise<Preview>((resolve,reject)=>{
        queue.push(()=>{void (async()=>{
          const data=await invoke<Background>('background_thumbnail',{key:path,archiveCandidate:archiveCandidate||null});
          const images=await backgroundImages(data),canvas=document.createElement('canvas');canvas.width=240;canvas.height=120;
          const context=canvas.getContext('2d');if(!context)throw Error('Canvas preview is unavailable');
          drawBackground(context,data,images);return {png:canvas.toDataURL('image/png'),state:data.state};
        })().then(resolve,reject).finally(()=>{active--;drain()})});
        drain();
      });
      previews.set(path,request);
      void request.catch(()=>{if(previews.get(path)===request)previews.delete(path)});
    }
    return request;
  };
}
function Thumbnail({path,selected,open,load}:{path:string;selected:boolean;open:(path:string)=>void;load:(path:string)=>Promise<Preview>}) {
  const button=useRef<HTMLButtonElement>(null);
  const [preview,setPreview]=useState<Preview|null>(null),[error,setError]=useState('');
  useEffect(()=>{
    let active=true;setPreview(null);setError('');
    const observer=new IntersectionObserver(entries=>{
      if(!entries.some(e=>e.isIntersecting))return;
      observer.disconnect();
      void load(path).then(value=>{if(active)setPreview(value)}).catch(e=>{if(active)setError(String(e))});
    },{rootMargin:'150px'});
    if(button.current)observer.observe(button.current);
    return()=>{active=false;observer.disconnect()};
  },[path,load]);
  return <button ref={button} className={'gallery-card '+(selected?'active':'')} onClick={()=>open(path)} title={error||`${path}${preview?' · State: '+preview.state:''}`}>
    <div className="gallery-thumbnail">{preview?<img src={preview.png} alt={path.split('/').pop()}/>:<span>{error?'Preview unavailable: '+error:'Rendering preview…'}</span>}</div>
    <strong>{path.split('/').pop()}</strong><small>{path.slice(0,path.lastIndexOf('/'))}</small>{preview&&<small>State: {preview.state}</small>}
  </button>;
}
export function BackgroundGallery({files,selected,open,archiveCandidate}:{files:string[];selected?:string;open:(path:string)=>void;archiveCandidate:string}) {
  const load=useMemo(()=>createLoader(archiveCandidate),[archiveCandidate]);
  return <div className="background-gallery" aria-label="Background gallery">
    {files.length?files.map(path=><Thumbnail key={path} path={path} selected={selected===path} open={open} load={load}/>):<p className="muted">No backgrounds match your search.</p>}
  </div>;
}
