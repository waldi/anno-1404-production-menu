import {useEffect,useState} from 'react';
import {invoke} from '@tauri-apps/api/core';
export type Glyph={Code:number;Width:number;Height:number;Advance:number;Bearing:number;UV:number[]};
export type Atlas={size:number[];cached_height:number;glyphs:Glyph[];alpha_png:string;outline_png:string|null;selected_face:string;scope:string};
export type Raster={size:number[];origin:number[];advance:number;png:string;scope:string};
function image(url:string):Promise<HTMLImageElement>{return new Promise((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(Error('Glyph mask failed to decode'));img.src=url})}
// Mask luminance is alpha coverage. Native atlas generation supplies pixel data;
// the browser does not choose fonts or rasterize the sample string.
export async function alphaImage(url:string,color:string){const img=await image(url);const canvas=document.createElement('canvas');canvas.width=img.width;canvas.height=img.height;const ctx=canvas.getContext('2d')!;ctx.drawImage(img,0,0);const pixels=ctx.getImageData(0,0,img.width,img.height);const rgb=color.match(/\w\w/g)!.map(c=>parseInt(c,16));for(let i=0;i<pixels.data.length;i+=4){const alpha=pixels.data[i];pixels.data[i]=rgb[0];pixels.data[i+1]=rgb[1];pixels.data[i+2]=rgb[2];pixels.data[i+3]=alpha}ctx.putImageData(pixels,0,0);return canvas}
export function NativeGlyphPanel({language,font,source}:{language:string;font:string;source:string}){
  const [atlas,setAtlas]=useState<Atlas|null>(null),[raster,setRaster]=useState<Raster|null>(null),[error,setError]=useState(''),[busy,setBusy]=useState(false),[text,setText]=useState('Anno 1404'),[color,setColor]=useState('ffffffff'),[code,setCode]=useState(65);
  useEffect(()=>{setAtlas(null);setError('')},[language,font,source]);
  async function load(){setBusy(true);setError('');setAtlas(null);try{setAtlas(await invoke<Atlas>('native_font_atlas',{language,font,source}))}catch(e){setError(String(e))}finally{setBusy(false)}}
  useEffect(()=>{let stale=false;setRaster(null);if(!atlas)return;void(async()=>{try{
    if(!/^[0-9a-f]{8}$/i.test(color))throw Error('Enter eight hexadecimal ARGB digits');
    const result=await invoke<Raster>('native_font_raster',{language,font,source,text,color:parseInt(color,16)});if(!stale){setRaster(result);setError('');}
  }catch(e){if(!stale)setError(String(e))}})();return()=>{stale=true}},[atlas,text,color,language,font,source]);
  const glyph=atlas?.glyphs[code];
  return <details><summary>Native glyph atlas</summary><button disabled={busy} onClick={()=>void load()}>Build atlas from this font source</button>{error&&<p className="warning">{error}</p>}{atlas&&<>
    <p>{atlas.selected_face} · {atlas.size.join(' × ')} atlas · cell height {atlas.cached_height}</p><p className="muted">Native GDI glyphs with recovered unit-scale outline and shadow composition. Color is an explicit sample input. Full caption layout, fractional filtering and translucent screenshot parity remain unverified.</p>
    <label>Text sample<input value={text} maxLength={128} onChange={e=>setText(e.target.value)}/></label><label>Sample color (ARGB)<input value={color} maxLength={8} onChange={e=>setColor(e.target.value)}/></label>{raster&&<><img src={raster.png} style={{maxWidth:'100%',background:'#202833'}}/><p className="muted">{raster.size.join(' × ')} px · advance {raster.advance} · origin {raster.origin.join(', ')}<br/>{raster.scope}</p></>}
    <label>Glyph code<input type="number" min={0} max={255} value={code} onChange={e=>setCode(Math.max(0,Math.min(255,Math.trunc(Number(e.target.value)||0))))}/></label>{glyph&&<p>Width {glyph.Width}, height {glyph.Height}, advance {glyph.Advance}, bearing {glyph.Bearing}<br/>UV {glyph.UV.join(', ')}</p>}
    <details><summary>Ink atlas mask</summary><img src={atlas.alpha_png} style={{maxWidth:'100%'}}/></details>{atlas.outline_png&&<details><summary>Outline atlas mask</summary><img src={atlas.outline_png} style={{maxWidth:'100%'}}/></details>}
  </>}</details>
}
