import React, { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Background, backgroundImages, drawBackground } from './background';
export function BackgroundPanel({ definition, states, thumbnail,archiveCandidate,setArchiveCandidate }: { definition: string; states: string[]; thumbnail: string | null;archiveCandidate:string;setArchiveCandidate:(value:string)=>void }) {
  const [state, setState] = useState(states[0] ?? ''), [width, setWidth] = useState(400), [height, setHeight] = useState(100);
  const [data, setData] = useState<Background>(), [error, setError] = useState(''), [png, setPng] = useState('');
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    let stale = false; setError(''); setData(undefined); setPng('');
    void invoke<Background>('background_render', { key: definition, layoutState: state, width, height,archiveCandidate:archiveCandidate||null }).then(async data => {
      const images = await backgroundImages(data); if (stale || !canvas.current) return;
      const target = canvas.current; target.width = width; target.height = height; const context = target.getContext('2d')!;
      context.clearRect(0, 0, width, height); drawBackground(context, data, images); setData(data); setPng(target.toDataURL('image/png'));
    }).catch(e => { if (!stale) setError(String(e)); });
    return () => { stale = true; };
  }, [definition, state, width, height,archiveCandidate]);
  return <div className="asset"><h2>Original-texture background renderer</h2><label>State <select aria-label="Background state" value={state} onChange={e => setState(e.target.value)}>{states.map(s => <option key={s}>{s}</option>)}</select></label>
    <label> Size <input aria-label="Background width" type="number" min="1" max="2048" value={width} onChange={e => setWidth(Math.max(1, Math.min(2048, +e.target.value)))} /> × <input aria-label="Background height" type="number" min="1" max="2048" value={height} onChange={e => setHeight(Math.max(1, Math.min(2048, +e.target.value)))} /></label>
    <label>Artwork candidate<select aria-label="Artwork candidate" value={archiveCandidate} onChange={e=>setArchiveCandidate(e.target.value)}><option value="">Unique extracted source</option><option value="startup-eng">Known English startup order · available assets</option><option value="maindata/patch0.rda">Staged patch 0 where available</option><option value="maindata/patch1.rda">Staged patch 1 where available</option></select></label>
    {error && <p role="alert">{error}</p>}<p>{data?.diagnostics.join(' ')}</p><canvas aria-label="Background tile canvas" ref={canvas} style={{ background: 'repeating-conic-gradient(#555 0% 25%,#333 0% 50%) 0/16px 16px', display: data ? 'inline-block' : 'none' }} />
    {data && <p>Client offsets: {data.client_offsets.join(', ')} · {data.tiles.length} tiles · <a href={png} download={definition.split('/').pop() + '-' + state + '.png'}>Export native PNG</a></p>}
    {data&&<details><summary>Resolved texture sources</summary>{Object.entries(data.sources??{}).map(([logical,source])=><p key={logical}>{logical}<br/>{source}</p>)}</details>}
    {thumbnail && <details><summary>Existing thumbnail — represented state unverified</summary><img src={thumbnail} alt="Extracted background thumbnail" /></details>}
  </div>;
}
