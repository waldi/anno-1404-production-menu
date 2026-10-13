import React, { useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { Catalog, View, Widget, Rect, prop, number, hit, gamePoint, movable, resizable, editable } from './model';
import './style.css';
import {InstallationSetup, InstallationStatus} from './InstallationSetup';
import { BackgroundGallery } from './BackgroundGallery';
import { ComparisonPanel } from './ComparisonPanel';
import { GeometryComparisonPanel } from './GeometryComparisonPanel';
import { IconPanel } from './IconPanel';
import { TextPanel } from './TextPanel';
import { TextMetricsPanel, TextFlowPanel } from './TextMetricsPanel';
import { ProgressInput, ProgressStatePanel } from './ProgressStatePanel';
import { TextRefreshInput, TextRefreshPanel } from './TextRefreshPanel';
import { LayoutScenario, LayoutPass, LayoutScenarioPanel } from './LayoutScenarioPanel';
import { ButtonStateComparisonPanel } from './ButtonStateComparisonPanel';
import { ButtonInput, ButtonSelection, ButtonStatePanel } from './ButtonStatePanel';
import { BackgroundPanel } from './BackgroundPanel';
import { Background, backgroundImages, drawBackground } from './background';
import { Atlas, Raster, alphaImage } from './NativeGlyphPanel';

function App({installation,onSetup}:{installation?:InstallationStatus;onSetup?:()=>void}) {
  const [changePending,setChangePending]=useState(false);
  const [catalog,setCatalog]=useState<Catalog|null>(null), [view,setView]=useState<View|null>(null), [selected,setSelected]=useState('0');
  const [query,setQuery]=useState(''), [kind,setKind]=useState('dlg'), [error,setError]=useState(''), [busy,setBusy]=useState(false);
  const [importPath,setImportPath]=useState('tools/production-menu/examples/production_dialog_v5.dlg');
  const [zoom,setZoom]=useState(.65), [width,setWidth]=useState(1280), [height,setHeight]=useState(1024), [hidden,setHidden]=useState(false), [grid,setGrid]=useState(4);
  const [cards,setCards]=useState<Record<string,number>>({}), [tab,setTab]=useState('properties'), [image,setImage]=useState<string|null>(null), [notice,setNotice]=useState('');
  const [progressInputs,setProgressInputs]=useState<Record<string,ProgressInput>>({});
  const [progressLayers,setProgressLayers]=useState<{id:string;role:string;destination:number[];clip:number[];background:Background|null;color:number|null;images:Record<string,HTMLImageElement>}[]>([]),[progressWarnings,setProgressWarnings]=useState<string[]>([]);
  const [archiveCandidate,setArchiveCandidate]=useState('startup-eng'); const [buttons,setButtons]=useState<Record<string,ButtonInput>>({});
  const [visibility,setVisibility]=useState<Record<string,boolean>>({});
  const [cityNameEnabled,setCityNameEnabled]=useState(false),[cityName,setCityName]=useState('');
  const [textRefresh,setTextRefresh]=useState<Record<string,TextRefreshInput>>({});
  const [layoutScenario,setLayoutScenario]=useState<LayoutScenario>({passes:1,island_info:null});
  const runtime=useMemo(()=>({current_city_name:cityNameEnabled?cityName:null,text_refresh:textRefresh,layout:layoutScenario}),[cityNameEnabled,cityName,textRefresh,layoutScenario]);
  const [skins,setSkins]=useState<Record<string,{data:Background;images:Record<string,HTMLImageElement>}>>({});
  const [iconSprites,setIconSprites]=useState<Record<string,{destination:number[];clip:number[];image:HTMLImageElement}>>({}),[iconWarnings,setIconWarnings]=useState<string[]>([]);
  const [captionSprites,setCaptionSprites]=useState<{id:string;destination:number[];clip:number[];image:HTMLImageElement}[]>([]),[captionWarnings,setCaptionWarnings]=useState<string[]>([]);
  const [skinWarnings,setSkinWarnings]=useState<string[]>([]),[guides,setGuides]=useState(true);
  const [preview,setPreview]=useState<{revision:number;rects:{id:string;parent:string|null;x:number;y:number;w:number;h:number;alpha:number;raw_relative:number[];effective_relative:number[];clip:[number,number,number,number];client:[number,number,number,number];diagnostics:string[]}[];diagnostics:string[];background_states:Record<string,ButtonSelection>;passes:LayoutPass[]}|null>(null);
  const [drag,setDrag]=useState<{rect:Rect; start:[number,number]; dx:number; dy:number; resize:boolean}|null>(null);
  const [gallery,setGallery]=useState(false);
  const lock=useRef(false),canvas=useRef<HTMLCanvasElement>(null);
  const native=!!(window as unknown as {__TAURI_INTERNALS__?:unknown}).__TAURI_INTERNALS__;
  const run=async<T,>(task:()=>Promise<T>,receive:(x:T)=>void)=>{ if(lock.current)return;lock.current=true;setBusy(true);setError('');try{receive(await task())}catch(e){setError(String(e))}finally{lock.current=false;setBusy(false)}};
  useEffect(()=>{if(native)void run(()=>invoke<Catalog>('catalog'),setCatalog);else setError('Desktop backend is required. Launch Anno GUI Studio.exe to load and edit the extracted assets.');},[]);
  useEffect(()=>{
    if(!(window as unknown as {__ANNO_SMOKE__?:boolean}).__ANNO_SMOKE__)return;
    void (async()=>{try {
      const c=await invoke<Catalog>('catalog');if(c.files.length!==1873||c.errors.length)throw Error('Native catalog check failed');setCatalog(c);
      let v=await invoke<View>('open_definition',{key:'menudefinitions/infobar.dlg'});if(v.tree?.name!=='INFOBAR')throw Error('Native hierarchy check failed');accept(v);
      const reference=await invoke<string>('smoke_reference');
      const localized=await invoke<{texts:{value:string}[];fonts:{source:string}[]}>('localisation_inspect',{language:'eng',guid:100001,font:'Tooltip'});
      if(!localized.fonts.length||!localized.texts.some(t=>t.value.includes('[AUTOSIZE_ICONGUID 122044 0]')))throw Error('Native localization inspection failed');
      const buttonReference=await invoke<string>('smoke_button_reference');
      const buttonComparison=await invoke<{counts:Record<string,number>}>('button_states_compare',{snapshot:buttonReference});
      if(buttonComparison.counts.exact!==53||buttonComparison.counts.mismatch)throw Error('Native captured button states failed');
      const metricReference=await invoke<string>('smoke_text_reference');
      const metricComparison=await invoke<{counts:Record<string,number>}>('text_metrics_compare',{snapshot:metricReference});
      if(metricComparison.counts.exact!==121||metricComparison.counts.mismatch)throw Error('Native captured text metrics failed');
      const flowReference=await invoke<string>('smoke_text_flow_reference');const flowComparison=await invoke<{exact:number;mismatch:number;unsupported:number}>('text_flow_compare',{snapshot:flowReference});
      if(flowComparison.exact!==13||flowComparison.mismatch||flowComparison.unsupported!==60)throw Error('Independent plain text placement comparison failed');
      const englishFlow=await invoke<{exact:number;mismatch:number;unsupported:number}>('text_flow_compare',{snapshot:flowReference,language:'eng'});if(englishFlow.exact!==22||englishFlow.mismatch||englishFlow.unsupported!==51)throw Error('Independent English GUID placement comparison failed');
      for(const [heldout,exact] of [[false,34],[true,33]] as const){
        const snapshot=await invoke<string>('smoke_text_inline_reference',{heldout});
        const result=await invoke<{exact:number;mismatch:number;unsupported:number}>('text_flow_compare',{snapshot,language:'eng'});
        if(result.exact!==exact||result.mismatch||result.unsupported!==39)throw Error('Independent inline text/icon placement comparison failed: '+JSON.stringify(result));
      }
      const metricFixture=JSON.parse(metricReference) as {fonts:{Address:number;Name:string;Face:string}[];runs:{Font:number;Text:string;Size:number[]}[]};
      const tooltipFont=metricFixture.fonts.find(f=>f.Name==='Tooltip')!;
      const tooltipRun=metricFixture.runs.find(r=>r.Font===tooltipFont.Address&&r.Text.length>0)!;
      const fontMetrics=await invoke<{size:number[];selected_face:string}>('native_font_metrics',{language:'eng',font:'Tooltip',source:localized.fonts[0].source,text:tooltipRun.Text,scale:[1,1]});
      if(fontMetrics.selected_face!==tooltipFont.Face||fontMetrics.size.join(',')!==tooltipRun.Size.join(','))throw Error('Native explicit font probe failed: '+JSON.stringify(fontMetrics));
      const glyphReference=await invoke<string>('smoke_glyph_reference');
      const glyphReport=await invoke<{exact:number;mismatch:number;unsupported:number}>('glyph_metadata_compare',{snapshot:glyphReference});if(glyphReport.exact!==5632||glyphReport.mismatch||glyphReport.unsupported)throw Error('Independent native glyph metadata check failed');
      const atlas=await invoke<Atlas>('native_font_atlas',{language:'eng',font:'Tooltip',source:localized.fonts[0].source});
      const liveGlyphs=(JSON.parse(glyphReference) as {fonts:{Name:string;Glyphs:Atlas['glyphs']}[]}).fonts.find(f=>f.Name==='Tooltip')!.Glyphs;
      if(atlas.glyphs.some((g,i)=>g.Width!==liveGlyphs[i].Width||g.Height!==liveGlyphs[i].Height||g.Advance!==liveGlyphs[i].Advance||g.Bearing!==liveGlyphs[i].Bearing||g.UV.join(',')!==liveGlyphs[i].UV.join(',')))throw Error('Explicit XML atlas disagrees with captured glyph metadata');
      const mask=await alphaImage(atlas.alpha_png,'ffffff');const rgba=mask.getContext('2d')!.getImageData(0,0,mask.width,mask.height).data;if(!rgba.some((v,i)=>i%4===3&&v>0)||!rgba.some((v,i)=>i%4===3&&v===0))throw Error('Native atlas mask/canvas decode failed');
      const sample=await invoke<Raster>('native_font_raster',{language:'eng',font:'Tooltip',source:localized.fonts[0].source,text:'Anno’s …',color:0xff010203});
      const sampleImage=await new Promise<HTMLImageElement>((resolve,reject)=>{const img=new Image();img.onload=()=>resolve(img);img.onerror=()=>reject(Error('Native glyph sample decode failed'));img.src=sample.png;});
      const sampleCanvas=document.createElement('canvas');sampleCanvas.width=sampleImage.width;sampleCanvas.height=sampleImage.height;const sampleContext=sampleCanvas.getContext('2d')!;sampleContext.drawImage(sampleImage,0,0);const samplePixels=sampleContext.getImageData(0,0,sampleCanvas.width,sampleCanvas.height).data;
      let opaqueSamplePixels=0;for(let i=0;i<samplePixels.length;i+=4){if(samplePixels[i+3]===255){opaqueSamplePixels++;if(samplePixels[i]!==1||samplePixels[i+1]!==2||samplePixels[i+2]!==3)throw Error('Native explicit sample color failed');}}if(!opaqueSamplePixels||sample.size.join(',')!==[sampleImage.width,sampleImage.height].join(','))throw Error('Native glyph sample dimensions failed');
      const menuFont=await invoke<{fonts:{source:string}[]}>('localisation_inspect',{language:'eng',guid:-1,font:'Mainmenu Subheader Start'});
      const menuSample=await invoke<Raster>('native_font_raster',{language:'eng',font:'Mainmenu Subheader Start',source:menuFont.fonts[0].source,text:'Save game',color:0xff010203});if(!menuSample.png.startsWith('data:image/png;base64,')||menuSample.advance<=0)throw Error('Document-retained menu shadow sample failed');
      const cityInput=await invoke<{runtime:{current_city_name:string|null};selection:{branch:string}|null}>('city_name_scenario',{language:'eng',json:JSON.stringify({current_city_name:'First Island'})});if(cityInput.runtime.current_city_name!=='First Island'||cityInput.selection!==null)throw Error('Explicit city scenario IPC failed');
      const syntheticNames=Array<string>(30).fill('');syntheticNames[0]='Synthetic first';syntheticNames[1]='Synthetic tie';const syntheticScores=Array<number>(8).fill(0);syntheticScores[0]=syntheticScores[1]=7;
      const selectedCity=await invoke<{runtime:{current_city_name:string|null};selection:{branch:string;player:number}}> ('city_name_scenario',{language:'eng',json:JSON.stringify({schema:3,source:{Player:29,Names:syntheticNames,PrimaryScores:syntheticScores,SecondaryCounts:Array<number>(30).fill(0)}})});if(selectedCity.runtime.current_city_name!=='Synthetic first'||selectedCity.selection.branch!=='primary-score'||selectedCity.selection.player!==0)throw Error('Synthetic city fallback IPC failed');
      const islandView=await invoke<View>('open_definition',{key:'menudefinitions/infobar_island_info.dlg'});
      const findIslandName=(node:Widget):Widget|undefined=>node.name==='ISLAND_NAME'?node:node.children.map(findIslandName).find(Boolean);
      const islandName=islandView.tree&&findIslandName(islandView.tree);if(!islandName)throw Error('Original island-name source missing');
      const islandLayout=await invoke<{rects:{id:string;raw_relative:number[]}[];text_sizes:Record<string,{client:number[]}>}>('preview_layout',{revision:islandView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng',runtime:cityInput.runtime});
      if(islandLayout.text_sizes[islandName.id]?.client.join(',')!=='90,22'||islandLayout.rects.find(r=>r.id===islandName.id)?.raw_relative.join(',')!=='11,9,101,31')throw Error('Source-driven label resize IPC disagrees with historical relative rectangle');
      const islandCaptions=await invoke<{captions:{id:string;parts:{text:string}[]}[];sprites:{id:string;png:string}[]}>('preview_captions',{revision:islandView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng',runtime:cityInput.runtime});
      if(islandCaptions.captions.find(c=>c.id===islandName.id)?.parts.map(p=>p.text).join('')!=='First Island'||!islandCaptions.sprites.some(s=>s.id===islandName.id&&s.png.startsWith('data:image/png;base64,')))throw Error('Resized city caption IPC failed');
      const loadView=await invoke<View>('open_definition',{key:'menudefinitions/load_text_frame.dlg'});const loadText=loadView.tree?.children[0];if(!loadText||number(loadText,'EXTENT/WIDTH')!==296)throw Error('Original loading label missing');
      const loadRuntime={text_refresh:{[loadText.id]:{client_width:280,fit_wrapped_width:false}}};
      const loadGeometry=await invoke<{rects:{id:string;raw_relative:number[]}[];text_sizes:Record<string,{client:number[]}>}>('preview_layout',{revision:loadView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng',runtime:loadRuntime});
      if(loadGeometry.text_sizes[loadText.id]?.client.join(',')!=='280,34'||loadGeometry.rects.find(r=>r.id===loadText.id)?.raw_relative.join(',')!=='36,30,332,80')throw Error('Explicit constrained refresh geometry IPC failed');
      const loadCaption=await invoke<{captions:{id:string;parts:unknown[]}[]}>('preview_captions',{revision:loadView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng',runtime:loadRuntime});
      const loadFit={text_refresh:{[loadText.id]:{client_width:280,fit_wrapped_width:true}}};
      const fittedGeometry=await invoke<{text_sizes:Record<string,{client:number[]}>}>('preview_layout',{revision:loadView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng',runtime:loadFit});
      const fittedCaption=await invoke<{captions:{id:string;parts:unknown[]}[]}>('preview_captions',{revision:loadView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng',runtime:loadFit});
      if(fittedGeometry.text_sizes[loadText.id]?.client.join(',')!=='259,34'||!loadCaption.captions.some(c=>c.id===loadText.id)||JSON.stringify(loadCaption.captions.find(c=>c.id===loadText.id)?.parts)!==JSON.stringify(fittedCaption.captions.find(c=>c.id===loadText.id)?.parts))throw Error('Constrained resize reflowed cached caption IPC');
      const saveView=await invoke<View>('open_definition',{key:'menudefinitions/save_dialog.dlg'});
      const captions=await invoke<{captions:{guid:number;parts:{text:string;size:number[];offset:number[];color:number}[]}[];sprites:{png:string}[]}>('preview_captions',{revision:saveView.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:{},buttons:{},language:'eng'});
      const title= captions.captions.find(c=>c.guid===122002);if(!title||title.parts.map(p=>p.text).join('')!=='Save game'||captions.captions.length!==3||captions.sprites.length!==9||!captions.sprites.every(s=>s.png.startsWith('data:image/png;base64,')))throw Error('Source caption native preview failed');
      const exitView=await invoke<View>('open_definition',{key:'menudefinitions/exit_game_menu.dlg'});
      const findCaptionButton=(node:Widget):Widget|undefined=>node.kind==='BUTTON'&&number(node,'BUTTON_TEXT_ENABLE')===1&&number(node,'TEXTSETTING/TEXTGUID',-1)>0?node:node.children.map(findCaptionButton).find(Boolean);
      const captionButton=exitView.tree&&findCaptionButton(exitView.tree);if(!captionButton)throw Error('Original caption button missing');
      const disabledCaption=await invoke<{captions:{id:string;parts:{color:number}[]}[];sprites:{id:string;png:string}[]}>('preview_captions',{revision:exitView.revision,width:1440,height:1080,showHidden:true,cards:{},visibility:{},buttons:{[captionButton.id]:{force_disabled:true}},language:'eng'});
      const disabledParts=disabledCaption.captions.find(c=>c.id===captionButton.id)?.parts;
      if(!disabledParts?.length||disabledParts.some(p=>p.color!==((number(captionButton,'DISABLEDTEXTCOLOR')|0xff000000)>>>0))||!disabledCaption.sprites.some(s=>s.id===captionButton.id&&s.png.startsWith('data:image/png;base64,')))throw Error('Button caption state/native IPC failed');
      v=await invoke<View>('open_definition',{key:'menudefinitions/infobar.dlg'});accept(v);
      const scenario=await invoke<{visibility:Record<string,boolean>}>('geometry_scenario',{snapshot:reference,revision:v.revision,width:1440,height:1080});
      const compared=await invoke<{counts:Record<string,number>}>('geometry_compare',{snapshot:reference,revision:v.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:scenario.visibility});
      if(compared.counts.exact!==25||compared.counts.mismatch)throw Error('Native live-reference geometry comparison failed');
      const icons=await invoke<{sprites:{destination:number[];icon:{png:string}}[]}>('preview_icons',{revision:v.revision,width:1440,height:1080,showHidden:false,cards:{},visibility:scenario.visibility});
      if(!icons.sprites.length||!icons.sprites.every(s=>s.destination[2]>0&&s.icon.png.startsWith('data:image/png;base64,')))throw Error('Native label icon placement failed');
      const edited=await invoke<View>('edit_properties',{revision:v.revision,id:'0',edits:[['EXTENT/WIDTH','1200']]});if(!edited.xml.includes('<WIDTH>1200</WIDTH>')||!edited.dirty)throw Error('Native edit check failed');accept(edited);
      const undone=await invoke<View>('history',{revision:edited.revision,redo:false});if(undone.xml!==v.xml)throw Error('Native undo check failed');accept(undone);
      const redone=await invoke<View>('history',{revision:undone.revision,redo:true});if(redone.xml!==edited.xml)throw Error('Native redo check failed');
      accept(await invoke<View>('history',{revision:redone.revision,redo:false}));
      const image=await invoke<string>('background_preview',{key:'backgrounds/final/objectmenu_frames/objectmenuinside.bkg'});if(!image.startsWith('data:image/png;base64,'))throw Error('Native asset check failed');
      await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
      if(!document.querySelector('canvas')?.width||!document.querySelector('[aria-label="WIDTH"]'))throw Error('Native rendered UI check failed');
      const galleryButton=document.querySelector<HTMLButtonElement>('.stage-toolbar button[aria-pressed]');
      if(!galleryButton)throw Error('Gallery toggle missing');galleryButton.click();
      const galleryDeadline=Date.now()+10000;
      while(Date.now()<galleryDeadline&&!document.querySelector<HTMLImageElement>('.gallery-thumbnail img')?.naturalWidth)await new Promise(resolve=>setTimeout(resolve,100));
      if(document.querySelectorAll('.gallery-card').length!==c.files.filter(f=>f.endsWith('.bkg')).length||!document.querySelector<HTMLImageElement>('.gallery-thumbnail img')?.naturalWidth)throw Error('Native gallery thumbnails failed');
      const galleryScroll=document.querySelector<HTMLElement>('.canvas-area')!;
      if(galleryScroll.clientHeight<=0||galleryScroll.scrollHeight<=galleryScroll.clientHeight||galleryScroll.getBoundingClientRect().bottom>window.innerHeight)throw Error('Gallery scroll container overflows viewport');
      galleryScroll.scrollTop=200;if(galleryScroll.scrollTop<=0)throw Error('Gallery cannot scroll');galleryScroll.scrollTop=0;
      galleryButton.click();await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
      const coin=await invoke<{png:string;source:number[]}>('icon_asset',{guid:1160,variation:0});if(coin.source.join(',')!=='0,506,46,46'||!coin.png.startsWith('data:image/png;base64,'))throw Error('Native icon crop failed');
      const bg=await invoke<Background>('background_render',{key:'backgrounds/final/objectmenu_frames/objectmenuinside.bkg',layoutState:'EmptyLayout',width:1080,height:114});
      const textures=await backgroundImages(bg), output=document.createElement('canvas');output.width=1080;output.height=114;const context=output.getContext('2d')!;drawBackground(context,bg,textures);
      if(!context.getImageData(36,36,1,1).data[3])throw Error('Native background pixels missing');
      await invoke('smoke_background',{png:output.toDataURL('image/png')});
      const checkedArt=await invoke<Background>('background_render',{key:'backgrounds/final/minimap_frames/force_north_button.bkg',layoutState:'Checked',width:24,height:24});
      const checkedImages=await backgroundImages(checkedArt),checkedCanvas=document.createElement('canvas');checkedCanvas.width=24;checkedCanvas.height=24;drawBackground(checkedCanvas.getContext('2d')!,checkedArt,checkedImages);
      await invoke('smoke_background',{png:checkedCanvas.toDataURL('image/png'),kind:'force-north-checked'});
      for(const version of [0,1]){const candidate=await invoke<Background>('background_render',{key:'backgrounds/final/minimap_frames/force_north_button.bkg',layoutState:'Checked',width:24,height:24,archiveCandidate:`maindata/patch${version}.rda`});const target=document.createElement('canvas');target.width=24;target.height=24;drawBackground(target.getContext('2d')!,candidate,await backgroundImages(candidate));await invoke('smoke_background',{png:target.toDataURL('image/png'),kind:`force-north-patch${version}`})}
      const startupArt=await invoke<Background>('background_render',{key:'backgrounds/final/minimap_frames/force_north_button.bkg',layoutState:'Checked',width:24,height:24,archiveCandidate:'startup-eng'});
      if(!Object.values(startupArt.sources??{}).some(path=>path.includes('maindata/patch1.rda/')))throw Error('Startup archive profile selected wrong minimap artwork');
      const startupIcon=await invoke<{png:string;texture_source:string}>('icon_asset',{guid:1160,variation:0,archiveCandidate:'startup-eng'});
      if(!startupIcon.texture_source||!startupIcon.png.startsWith('data:image/png;base64,'))throw Error('Startup icon source identity missing');
      const loading=await invoke<View>('open_definition',{key:'menudefinitions/loading_screen.dlg'});
      const progressNodes:Widget[]=[];const collectProgress=(w:Widget)=>{if(w.kind==='PROGRESSBAR')progressNodes.push(w);w.children.forEach(collectProgress)};collectProgress(loading.tree!);
      if(!progressNodes.length)throw Error('Progress fixture has no widgets');
      const progress=await invoke<{layers:{role:string;destination:number[];background:Background|null}[]}>('preview_progress',{revision:loading.revision,width:1280,height:1024,showHidden:true,cards:{},visibility:{},buttons:{},archiveCandidate:'startup-eng',inputs:Object.fromEntries(progressNodes.map(w=>[w.id,{current:50,maximum:100}]))});
      const bar=progress.layers.find(l=>l.role==='PB_Bar'&&l.background);
      if(!bar?.background||bar.destination[2]<=bar.destination[0])throw Error('Native progress layers failed');
      const progressCanvas=document.createElement('canvas');progressCanvas.width=bar.destination[2]-bar.destination[0];progressCanvas.height=bar.destination[3]-bar.destination[1];drawBackground(progressCanvas.getContext('2d')!,bar.background,await backgroundImages(bar.background));
      await invoke('smoke_background',{png:progressCanvas.toDataURL('image/png'),kind:'progress'});
      const unchanged=await invoke<View>('current_view');if(unchanged.xml!==loading.xml||unchanged.dirty)throw Error('Progress scenario modified source');
      accept(await invoke<View>('open_definition',{key:'backgrounds/final/minimap_frames/force_north_button.bkg'}));
      await new Promise<void>(resolve=>requestAnimationFrame(()=>requestAnimationFrame(()=>resolve())));
      const artworkChoice=document.querySelector<HTMLSelectElement>('[aria-label="Artwork candidate"]');if(!artworkChoice)throw Error('Artwork candidate selector missing');artworkChoice.value='maindata/patch1.rda';artworkChoice.dispatchEvent(new Event('change',{bubbles:true}));
      const artworkDeadline=Date.now()+10000;while(Date.now()<artworkDeadline&&!document.querySelector('.asset')?.textContent?.includes('Explicit maindata/patch1.rda'))await new Promise(resolve=>setTimeout(resolve,100));
      if(!document.querySelector('.asset')?.textContent?.includes('Explicit maindata/patch1.rda')||!document.querySelector<HTMLCanvasElement>('[aria-label="Background tile canvas"]')?.width)throw Error('Artwork candidate UI rendering failed');
      await invoke('smoke_complete',{success:true,message:'Packaged WebView/Tauri IPC passed catalog, hierarchy, edits, history, gallery, 25-widget geometry, icon crops, 121 native text metrics, 53 button-state comparisons and progress layer canvas composition. No sources or working copies saved. Full pixel fidelity remains unverified.'});
    }catch(e){await invoke('smoke_complete',{success:false,message:String(e)})}})();
  },[]);
  const all=useMemo(()=>{const nodes:Widget[]=[];function walk(n:Widget){nodes.push(n);n.children.forEach(walk)}if(view?.tree)walk(view.tree);return nodes},[view]);
  const node=all.find(n=>n.id===selected)??view?.tree;
  useEffect(()=>{let stale=false;setPreview(null);if(native&&view?.tree)invoke<NonNullable<typeof preview>>('preview_layout',{revision:view.revision,width,height,showHidden:hidden,cards,visibility,buttons,language:"eng",runtime}).then(p=>{if(!stale)setPreview(p)}).catch(e=>{if(!stale)setError(String(e))});return()=>{stale=true}},[view?.revision,width,height,hidden,cards,visibility,buttons,archiveCandidate,runtime]);
  const rects=useMemo(()=>preview?.revision===view?.revision?preview?.rects.flatMap(r=>{const node=all.find(n=>n.id===r.id);return node?[{...r,node,parent:all.find(n=>n.id===r.parent)??null}]:[]})??[]:[],[preview,view?.revision,all]);
  useEffect(()=>{let stale=false;setCaptionSprites([]);setCaptionWarnings([]);if(native&&view?.tree)void invoke<{sprites:{id:string;destination:number[];clip:number[];png:string}[];diagnostics:string[]}>('preview_captions',{revision:view.revision,width,height,showHidden:hidden,cards,visibility,buttons,language:'eng',runtime}).then(async result=>{
    const sprites=await Promise.all(result.sprites.map(async sprite=>{const image=new Image();image.src=sprite.png;await image.decode();return {...sprite,image}}));if(!stale){setCaptionSprites(sprites);setCaptionWarnings(result.diagnostics)}
  }).catch(e=>{if(!stale)setCaptionWarnings([String(e)])});return()=>{stale=true}},[view?.revision,width,height,hidden,cards,visibility,buttons,runtime]);
  useEffect(()=>{let stale=false;setIconSprites({});setIconWarnings([]);if(native&&view?.tree)void invoke<{sprites:{id:string;destination:number[];clip:number[];icon:{png:string}}[];diagnostics:string[]}>('preview_icons',{revision:view.revision,width,height,showHidden:hidden,cards,visibility,buttons,language:"eng",runtime,archiveCandidate:archiveCandidate||null}).then(async result=>{
    const sprites:Record<string,{destination:number[];clip:number[];image:HTMLImageElement}>={};
    await Promise.all(result.sprites.map(async sprite=>{const image=new Image();image.src=sprite.icon.png;await image.decode();sprites[sprite.id]={...sprite,image}}));
    if(!stale){setIconSprites(sprites);setIconWarnings(result.diagnostics)}
  }).catch(e=>{if(!stale)setIconWarnings([String(e)])});return()=>{stale=true}},[view?.revision,width,height,hidden,cards,visibility,buttons,archiveCandidate,runtime]);
  useEffect(()=>{let stale=false;setSkins({});setSkinWarnings([]);const requests=new Map<string,Promise<{data:Background;images:Record<string,HTMLImageElement>}>>();
    void Promise.all(rects.filter(r=>r.w>0&&r.h>0&&prop(r.node,'BACKGROUND')).map(async r=>{const logical=prop(r.node,'BACKGROUND').replaceAll('\\','/').split('/config/gui/')[1];if(!logical)return null;const state=preview?.background_states[r.node.id]?.selected??'EmptyLayout';const signature=logical+':'+state+':'+r.w+':'+r.h;let request=requests.get(signature);if(!request){request=invoke<Background>('background_render',{key:logical,layoutState:state,width:r.w,height:r.h,archiveCandidate:archiveCandidate||null}).then(async data=>({data,images:await backgroundImages(data)}));requests.set(signature,request);}try{return {id:r.node.id,skin:await request}}catch(e){return {id:r.node.id,error:String(e)}}})).then(results=>{if(stale)return;const next:typeof skins={};const warnings:string[]=[];for(const r of results){if(!r)continue;if('skin' in r&&r.skin)next[r.id]=r.skin;else if('error' in r)warnings.push(r.id+': '+r.error);}setSkins(next);setSkinWarnings(warnings);});return()=>{stale=true};
  },[rects,archiveCandidate]);
  useEffect(()=>{let stale=false;setProgressLayers([]);setProgressWarnings([]);if(native&&view?.tree)void invoke<{layers:Omit<(typeof progressLayers)[number],'images'>[];diagnostics:string[]}>('preview_progress',{revision:view.revision,width,height,showHidden:hidden,cards,visibility,buttons,language:"eng",runtime,inputs:progressInputs,archiveCandidate:archiveCandidate||null}).then(async result=>{const layers=await Promise.all(result.layers.map(async layer=>({...layer,images:layer.background?await backgroundImages(layer.background):{}})));if(!stale){setProgressLayers(layers);setProgressWarnings(result.diagnostics)}}).catch(e=>{if(!stale)setProgressWarnings([String(e)])});return()=>{stale=true}},[view?.revision,width,height,hidden,cards,visibility,buttons,progressInputs,archiveCandidate,runtime]);
  const current=rects.find(r=>r.node.id===selected);
  const accept=(v:View)=>{setView(v);setNotice(v.saved_to?'Working copy: '+v.saved_to:'Original source · edits stay in memory until Save copy')};
  const acceptOpen=(v:View)=>{accept(v);setSelected('0');setImage(null);setCards({});setVisibility({});setButtons({});setProgressInputs({});setTextRefresh({});setLayoutScenario({passes:1,island_info:null});setTab('properties');setGallery(false)};
  const open=(key:string)=>{if(view?.dirty&&!confirm('Discard unsaved edits and open another definition?'))return;void run(()=>invoke<View>('open_definition',{key}),acceptOpen)};
  const openRepoDefinition=()=>{if(view?.dirty&&!confirm('Discard unsaved edits and open another definition?'))return;void run(()=>invoke<View>('import_definition',{relativePath:importPath}),v=>{acceptOpen(v);setNotice('Imported source snapshot opened. Save copy writes only to the GUI studio workspace.');void invoke<Catalog>('catalog').then(setCatalog)})};
  const chooseRepoDefinition=()=>{void run(()=>invoke<string|null>('choose_definition_file'),path=>{if(path)setImportPath(path)})};
  const edits=(entries:[string,string|null][])=>{if(!view||!node)return;void run(()=>invoke<View>('edit_properties',{revision:view.revision,id:node.id,edits:entries}),accept)};
  const history=(redo:boolean)=>{if(view)void run(()=>invoke<View>('history',{revision:view.revision,redo}),accept)};
  const save=()=>{if(view)void run(()=>invoke<View>('save_copy',{revision:view.revision}),v=>{accept(v);setNotice('Saved working copy: '+v.saved_to)})};
  useEffect(()=>{
    const key=node?prop(node,'BACKGROUND'):'';const logical=key.replaceAll('\\','/').split('/config/gui/')[1];setImage(null);let stale=false;
    if(native&&logical&&catalog?.files.includes(logical))invoke<string>('background_preview',{key:logical}).then(x=>{if(!stale)setImage(x)}).catch(()=>{});
    return()=>{stale=true};
  },[node,view?.revision]);
  useEffect(()=>{
    const c=canvas.current;if(!c)return;const dpr=window.devicePixelRatio||1;c.width=width*zoom*dpr;c.height=height*zoom*dpr;
    const ctx=c.getContext('2d')!;ctx.scale(zoom*dpr,zoom*dpr);ctx.fillStyle='#111a22';ctx.fillRect(0,0,width,height);
    ctx.strokeStyle='#21313d';ctx.lineWidth=1/zoom;ctx.beginPath();for(let x=0;x<width;x+=32){ctx.moveTo(x,0);ctx.lineTo(x,height)}for(let y=0;y<height;y+=32){ctx.moveTo(0,y);ctx.lineTo(width,y)}ctx.stroke();
    for(const r of rects){let{x,y,w,h}=r;if(drag?.rect.node.id===r.node.id){if(drag.resize){w=Math.max(1,w+drag.dx);h=Math.max(1,h+drag.dy)}else{x+=drag.dx;y+=drag.dy}}
      ctx.save();ctx.beginPath();ctx.rect(r.clip[0],r.clip[1],Math.max(0,r.clip[2]-r.clip[0]),Math.max(0,r.clip[3]-r.clip[1]));ctx.clip();ctx.globalAlpha=r.diagnostics.some(d=>d.startsWith('Hidden widget shown'))?.35:r.alpha;
      const skin=skins[r.node.id];if(skin)drawBackground(ctx,skin.data,skin.images,x,y);
      ctx.restore();
      for(const layer of progressLayers.filter(p=>p.id===r.node.id)){ctx.save();ctx.beginPath();ctx.rect(layer.clip[0],layer.clip[1],Math.max(0,layer.clip[2]-layer.clip[0]),Math.max(0,layer.clip[3]-layer.clip[1]));ctx.clip();ctx.globalAlpha=layer.role==='flat'?1:r.alpha;const d=layer.destination;if(layer.background)drawBackground(ctx,layer.background,layer.images,d[0]+x-r.x,d[1]+y-r.y);else if(layer.color!==null){ctx.fillStyle='#'+(layer.color&0xffffff).toString(16).padStart(6,'0');ctx.fillRect(d[0]+x-r.x,d[1]+y-r.y,d[2]-d[0],d[3]-d[1])}ctx.restore()}
      const sprite=iconSprites[r.node.id];if(sprite){ctx.save();ctx.beginPath();ctx.rect(sprite.clip[0],sprite.clip[1],Math.max(0,sprite.clip[2]-sprite.clip[0]),Math.max(0,sprite.clip[3]-sprite.clip[1]));ctx.clip();ctx.globalAlpha=r.alpha;const d=sprite.destination;ctx.drawImage(sprite.image,d[0]+x-r.x,d[1]+y-r.y,d[2],d[3]);ctx.restore()}
      for(const caption of captionSprites.filter(c=>c.id===r.node.id)){ctx.save();ctx.beginPath();ctx.rect(caption.clip[0],caption.clip[1],Math.max(0,caption.clip[2]-caption.clip[0]),Math.max(0,caption.clip[3]-caption.clip[1]));ctx.clip();ctx.globalAlpha=1;const d=caption.destination;ctx.drawImage(caption.image,d[0]+x-r.x,d[1]+y-r.y,d[2],d[3]);ctx.restore()}
      ctx.save();ctx.beginPath();ctx.rect(r.clip[0],r.clip[1],Math.max(0,r.clip[2]-r.clip[0]),Math.max(0,r.clip[3]-r.clip[1]));ctx.clip();
      if(guides){ctx.fillStyle=r.node.kind==='BUTTON'?'#263d49':r.node.kind==='FRAME'?'#15212baa':'#28414b55';if(!skin)ctx.fillRect(x,y,w,h);ctx.strokeStyle=r.node.id===selected?'#ffce7a':'#547580';ctx.lineWidth=(r.node.id===selected?2:1)/zoom;ctx.strokeRect(x,y,w,h);
      if(w>20&&h>12){ctx.save();ctx.beginPath();ctx.rect(x,y,w,h);ctx.clip();ctx.fillStyle='#b0c5ce';ctx.font='12px Segoe UI';ctx.fillText(r.node.name||r.node.kind,x+5,y+15);ctx.restore()}}
      if(r.node.id===selected&&resizable(r)){ctx.fillStyle='#ffce7a';ctx.fillRect(x+w-6/zoom,y+h-6/zoom,8/zoom,8/zoom)}ctx.restore();
    }
  },[rects,selected,zoom,width,height,drag,skins,iconSprites,captionSprites,progressLayers,guides,gallery]);
  useEffect(()=>{const handler=(e:KeyboardEvent)=>{if((e.target as HTMLElement).matches('input,textarea,select'))return;if(e.ctrlKey&&e.key.toLowerCase()==='s'){e.preventDefault();save()}else if(e.ctrlKey&&e.key.toLowerCase()==='z'){e.preventDefault();history(e.shiftKey)}else if(current&&movable(current)&&['ArrowLeft','ArrowRight','ArrowUp','ArrowDown'].includes(e.key)){e.preventDefault();const step=e.shiftKey?10:1;edits([['EXTENT/LEFT',String(number(current.node,'EXTENT/LEFT')+(e.key==='ArrowLeft'?-step:e.key==='ArrowRight'?step:0))],['EXTENT/TOP',String(number(current.node,'EXTENT/TOP')+(e.key==='ArrowUp'?-step:e.key==='ArrowDown'?step:0))]])}};window.addEventListener('keydown',handler);return()=>window.removeEventListener('keydown',handler)},[view,current]);
  const pointer=(e:React.PointerEvent)=>{const r=e.currentTarget.getBoundingClientRect();return gamePoint(e.clientX,e.clientY,r.left,r.top,zoom)};
  const down=(e:React.PointerEvent<HTMLCanvasElement>)=>{if(busy)return;const point=pointer(e),r=hit(rects,...point);if(!r)return;setSelected(r.node.id);const resize=point[0]>r.x+r.w-12/zoom&&point[1]>r.y+r.h-12/zoom&&resizable(r);if(resize||movable(r)){e.currentTarget.setPointerCapture(e.pointerId);setDrag({rect:r,start:point,dx:0,dy:0,resize})}};
  const move=(e:React.PointerEvent<HTMLCanvasElement>)=>{if(!drag)return;const p=pointer(e),snap=(n:number)=>Math.round(n/Math.max(1,grid))*Math.max(1,grid);setDrag({...drag,dx:snap(p[0]-drag.start[0]),dy:snap(p[1]-drag.start[1])})};
  const up=()=>{if(!drag||!view)return;const d=drag;setDrag(null);if(d.dx===0&&d.dy===0)return;const p=d.resize?[['EXTENT/WIDTH',String(Math.max(1,number(d.rect.node,'EXTENT/WIDTH')+d.dx))],['EXTENT/HEIGHT',String(Math.max(1,number(d.rect.node,'EXTENT/HEIGHT')+d.dy))]]:[['EXTENT/LEFT',String(number(d.rect.node,'EXTENT/LEFT')+d.dx)],['EXTENT/TOP',String(number(d.rect.node,'EXTENT/TOP')+d.dy)]];void run(()=>invoke<View>('edit_properties',{revision:view.revision,id:d.rect.node.id,edits:p}),accept)};
  function tree(n:Widget,depth=0):React.ReactNode{return <React.Fragment key={n.id}><button className={'tree-node '+(selected===n.id?'active':'')} style={{paddingLeft:12+depth*14}} onClick={()=>setSelected(n.id)}><span className="type">{n.kind}</span>{n.name||'Unnamed'}</button>{n.children.map(c=>tree(c,depth+1))}</React.Fragment>}
  const files=(catalog?.files??[]).filter(f=>(kind==='all'||f.endsWith('.'+kind))&&f.toLowerCase().includes(query.toLowerCase()));
  return <div className="app">{changePending&&<div className="setup-overlay"><section className="installation-setup"><h2>Save your edits before changing installation?</h2><div className="setup-actions"><button onClick={()=>setChangePending(false)}>Cancel</button><button onClick={()=>{setChangePending(false);onSetup?.()}}>Discard and continue</button><button className="primary" disabled={busy} onClick={()=>void run(()=>invoke<View>('save_copy',{revision:view!.revision}),v=>{accept(v);setChangePending(false);onSetup?.()})}>Save and continue</button></div></section></div>}<header><div className="brand"><span className="mark">A</span><div><strong>Anno GUI Studio</strong><small>ANNO 1404 GUI DEFINITIONS</small></div></div>{installation&&<button className="installation-path" title={`Game: ${installation.selection.game}\nRDA Explorer: ${installation.selection.explorer}`} onClick={()=>{if(view?.dirty)setChangePending(true);else onSetup?.()}}>Game directory: {installation.selection.game.replace(/^.*[\\/]/,'')} · Change…</button>}<span className="document">{view?view.key.split('/').pop():'Choose a definition'}{view?.dirty?' •':''}</span><div className="actions"><label>Artwork<select aria-label="Dialog artwork candidate" value={archiveCandidate} onChange={e=>setArchiveCandidate(e.target.value)}><option value="">Unique extracted source</option><option value="startup-eng">Known English startup order · available assets</option><option value="maindata/patch0.rda">Staged patch 0 where available</option><option value="maindata/patch1.rda">Staged patch 1 where available</option></select></label><button disabled={!view?.can_undo||busy} onClick={()=>history(false)}>Undo</button><button disabled={!view?.can_redo||busy} onClick={()=>history(true)}>Redo</button><button className="primary" disabled={!view?.tree||busy} onClick={save}>Save copy</button></div></header>
    {error&&<div role="alert" className="error">{error}<button onClick={()=>setError('')}>×</button></div>}
    <main><aside className="browser"><div className="section-title">DEFINITIONS <span>{catalog?.files.length??0}</span></div><input aria-label="Search definitions" placeholder="Search definitions…" value={query} onChange={e=>setQuery(e.target.value)}/><div className="filters">{['dlg','tpl','bkg','all'].map(k=><button className={kind===k?'active':''} key={k} onClick={()=>{setKind(k);setGallery(k==='bkg')}}>{k==='all'?'All':'.'+k}</button>)}</div><div className="repo-import"><label>Open a repo-relative or selected definition<input aria-label="Definition path" value={importPath} onChange={e=>setImportPath(e.target.value)}/></label><button disabled={busy} onClick={chooseRepoDefinition}>Choose .dlg/.tpl…</button><button disabled={busy||!importPath.trim()} onClick={openRepoDefinition}>Open as working copy</button><p className="muted">Imports .dlg/.tpl files as a private source snapshot. The source stays untouched; Save writes under the GUI studio workspace.</p></div><div className="file-list">{files.map(f=><button key={f} title={f} className={view?.key===f?'active':''} onClick={()=>open(f)}>{f.split('/').pop()}</button>)}</div><div className="section-title">WIDGET TREE <span>{all.length}</span></div><div className="hierarchy">{view?.tree?tree(view.tree):<p className="muted">Open a dialog or template.</p>}</div></aside>
    <section className="stage"><div className="stage-toolbar"><span>{gallery?'Background gallery':'Layout preview'}</span><button aria-pressed={gallery} onClick={()=>setGallery(!gallery)}>Background gallery</button><label>Viewport <input aria-label="Viewport width" type="number" min="100" max="4096" value={width} onChange={e=>setWidth(Math.max(100,Math.min(4096,+e.target.value)))}/> × <input aria-label="Viewport height" type="number" min="100" max="4096" value={height} onChange={e=>setHeight(Math.max(100,Math.min(4096,+e.target.value)))}/></label><label>Zoom <select aria-label="Zoom" value={zoom} onChange={e=>setZoom(+e.target.value)}>{[.25,.5,.65,.75,1,1.5,2].map(z=><option value={z} key={z}>{Math.round(z*100)}%</option>)}</select></label></div><div className="preview-note">{gallery?'Rendered background previews · first defined state at 240 × 120 · click to inspect source and states':'Rust geometry · original-texture backgrounds · source English LABEL/BUTTON captions at unit scale · runtime text/state/effects remain unverified'}</div><div className="canvas-area">{gallery?<BackgroundGallery files={(catalog?.files??[]).filter(f=>f.endsWith('.bkg')&&f.toLowerCase().includes(query.toLowerCase()))} selected={view?.key} open={open} archiveCandidate={archiveCandidate}/>:view?.tree?<canvas ref={canvas} style={{width:width*zoom,height:height*zoom}} onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={()=>setDrag(null)} aria-label="Dialog layout canvas"/>:view?.key.endsWith('.bkg')?<BackgroundPanel key={view.key} definition={view.key} states={view.states} thumbnail={view.preview} archiveCandidate={archiveCandidate} setArchiveCandidate={setArchiveCandidate}/>:<div className="empty"><span className="empty-icon">▦</span><h1>Explore. Arrange. Refine.</h1><p>Open a dialog to inspect its widget tree and edit a separate working copy.</p><p className="muted">Original extracted assets stay untouched.</p></div>}</div><div className="stage-footer"><label><input type="checkbox" checked={hidden} onChange={e=>setHidden(e.target.checked)}/> Show hidden</label><label><input type="checkbox" checked={guides} onChange={e=>setGuides(e.target.checked)}/> Widget guides</label><label title="Explicit resolved display name for CURRENT_CITY_NAME; preview only"><input type="checkbox" checked={cityNameEnabled} onChange={e=>setCityNameEnabled(e.target.checked)}/> City name scenario <input aria-label="City name scenario" disabled={!cityNameEnabled} value={cityName} maxLength={512} onChange={e=>setCityName(e.target.value)}/></label><label>Import preview scenario <input aria-label="Import preview scenario" type="file" accept=".json,application/json" onChange={async e=>{const file=e.target.files?.[0];e.target.value="";if(!file)return;try{const imported=await invoke<{runtime:{current_city_name:string|null;text_refresh:Record<string,TextRefreshInput>;layout:LayoutScenario};selection:{branch:string}|null;scope:string}>("city_name_scenario",{json:await file.text(),language:"eng"});setTextRefresh(imported.runtime.text_refresh??{});setLayoutScenario(imported.runtime.layout??{passes:1,island_info:null});setCityName(imported.runtime.current_city_name??"");setCityNameEnabled(imported.runtime.current_city_name!==null);setNotice(imported.selection?"City name scenario: "+imported.selection.branch:imported.scope)}catch(error){setError(String(error))}}}/></label><label>Snap <input aria-label="Grid snap" type="number" min="1" max="64" value={grid} onChange={e=>setGrid(Math.max(1,+e.target.value))}/> px</label><span>{busy?'Working…':current?`${current.node.kind} · ${current.w} × ${current.h}`:'Ready'}</span></div></section>
    <aside className="inspector"><div className="section-title">INSPECTOR</div><h2>{node?.name||node?.kind||'No selection'}</h2><div className="inspector-tabs">{['properties','source','changes','compare'].map(t=><button className={tab===t?'active':''} key={t} onClick={()=>setTab(t)}>{t}</button>)}</div><div className="inspector-content">{tab==='properties'&&node&&<><div className="geometry">{['LEFT','TOP','WIDTH','HEIGHT'].map(k=>{const key='EXTENT/'+k;const p=node.properties.find(p=>p.key===key);const controlled=current&&((['LEFT','TOP'].includes(k)&&!movable(current))||(['WIDTH','HEIGHT'].includes(k)&&!resizable(current)));return <label key={k}>{k}<input aria-label={k} key={view?.revision+'-'+key} disabled={busy||!!controlled} defaultValue={p?.value??''} placeholder="Inherited / unset" onBlur={e=>{if(e.target.value!==(p?.value??''))edits([[key,e.target.value]])}}/></label>})}</div>{current&&!movable(current)&&<p className="muted">Position is controlled by the parent layout or alignment.</p>}{node.kind==='CARDLAYOUT'&&<label>Preview card<select aria-label="Preview card" value={cards[node.id]??0} onChange={e=>setCards({...cards,[node.id]:+e.target.value})}>{node.children.map((c,i)=><option key={c.id} value={i}>{c.name||c.kind}</option>)}</select></label>}<label>Scenario visibility<select aria-label="Scenario visibility" value={visibility[node.id]===undefined?'source':visibility[node.id]?'show':'hide'} onChange={e=>{const next={...visibility};if(e.target.value==='source')delete next[node.id];else next[node.id]=e.target.value==='show';setVisibility(next)}}><option value="source">From source</option><option value="show">Visible</option><option value="hide">Hidden</option></select></label><details><summary>Preview coverage ({skinWarnings.length+iconWarnings.length+captionWarnings.length+progressWarnings.length} diagnostics)</summary>{preview?.diagnostics.map((d,i)=><p key={i}>{d}</p>)}{skinWarnings.concat(iconWarnings,captionWarnings,progressWarnings).map((d,i)=><p key={i}>{d}</p>)}</details><p className="muted">Inherited values are candidates. Editing creates a local override.</p>{current&&'diagnostics' in current&&(current.diagnostics as string[]).map((d,i)=><p className="warning" key={i}>{d}</p>)}{node.warnings.map((w,i)=><p className="warning" key={i}>{w}</p>)}{node.properties.filter(p=>!p.key.startsWith('EXTENT/')).map((p,i)=><div className="property" key={p.key}><label>{p.key}<span className={p.local?'local':p.source.startsWith('engine:')?'constructor':'inherited'}>{p.local?'local':p.source.startsWith('engine:')?'constructor':'inherited'}</span></label><div className="property-value"><input aria-label={p.key} key={view?.revision+'-'+i} disabled={busy||!editable(p.key)} defaultValue={p.value} onBlur={e=>{if(e.target.value!==p.value)edits([[p.key,e.target.value]])}}/>{p.local&&editable(p.key)&&<button title="Remove local override" disabled={busy} onClick={()=>edits([[p.key,null]])}>↶</button>}</div>{!p.local&&!p.source.startsWith('engine:')&&<button className="source-link" onClick={()=>open(p.source)}>{p.source.split('/').pop()}</button>}</div>)}{prop(node,'ICONGUID')&&number(node,'ICONGUID')>0&&<IconPanel key={node.id} guid={number(node,'ICONGUID')} archiveCandidate={archiveCandidate}/>}{image&&<div className="background-thumbnail"><img src={image} alt="Selected widget background"/><small>Background asset thumbnail</small></div>}<>{node.kind==="BUTTON"&&<ButtonStatePanel value={buttons[node.id]??{hovered:false,pressed:false,checked:false}} selection={preview?.background_states[node.id]} onChange={value=>setButtons({...buttons,[node.id]:value})}/>}</><>{node.kind==="PROGRESSBAR"&&<ProgressStatePanel value={progressInputs[node.id]??{}} current={prop(node,"CURRENTVALUE")} maximum={prop(node,"MAXVALUE")} onChange={value=>setProgressInputs({...progressInputs,[node.id]:value})}/>}</>{node.kind==='LABEL'&&number(node,'TEXTSETTING/TEXTGUID',-1)>0&&<TextRefreshPanel value={textRefresh[node.id]} onChange={value=>{const next={...textRefresh};if(value)next[node.id]=value;else delete next[node.id];setTextRefresh(next)}}/>}{node.id===view?.tree?.id&&<LayoutScenarioPanel root={node} width={width} height={height} value={layoutScenario} onChange={setLayoutScenario} passes={preview?.passes??[]}/>}<TextPanel key={node.id} guid={prop(node,'TEXTSETTING/TEXTGUID')} font={prop(node,'TEXTSETTING/FONT')||prop(node,'FONT')}/><details><summary>Ordered template layers</summary><pre>{JSON.stringify(node.layers,null,2)}</pre></details></>}{tab==='compare'&&<>{view?.tree&&<GeometryComparisonPanel view={view} width={width} height={height} hidden={hidden} cards={cards} visibility={visibility} buttons={buttons} runtime={runtime} onVisibility={setVisibility}/>}<TextMetricsPanel/><TextFlowPanel/><ButtonStateComparisonPanel/><ComparisonPanel/></>}{tab==='source'&&<pre className="xml">{view?.xml??'No source loaded'}</pre>}{tab==='changes'&&<><p className="muted">Original → current working copy</p>{view?.xml===view?.original?<p>No changes from original.</p>:<><h3>Original</h3><pre>{view?.original}</pre><h3>Working copy</h3><pre>{view?.xml}</pre></>}</>}</div></aside></main><footer><span>{notice||'Local desktop editor · separate working copies'}</span><span>{catalog?.errors.length?catalog.errors.length+' parse errors':'SOURCE ASSETS READ-ONLY'}</span></footer></div>;
}
function Studio() {
  const smoke=!!(window as unknown as {__ANNO_SMOKE__?:boolean}).__ANNO_SMOKE__;
  const [status,setStatus]=useState<InstallationStatus|null>(null),[settings,setSettings]=useState(false),[generation,setGeneration]=useState(0),[error,setError]=useState('');
  useEffect(()=>{
    if(smoke)return;
    void (async()=>{
      const setupSmoke=!!(window as unknown as {__ANNO_SETUP_SMOKE__?:boolean}).__ANNO_SETUP_SMOKE__;
      try {
        let s=await invoke<InstallationStatus>('installation_status');setStatus(s);
        if(!setupSmoke)return;
        const frame=()=>new Promise<void>(resolve=>setTimeout(resolve,200));await frame();
        if(!s.ready){
          if(!document.querySelector('[aria-label="Game directory"]')||!document.querySelector('[aria-label="RDA Explorer directory"]'))throw Error('First-launch folder controls missing');
          const [game,explorer]=await invoke<[string,string]>('setup_smoke_folders');
          let rejected=false;try{await invoke('prepare_installation',{game:'missing-directory',explorer})}catch{rejected=true}if(!rejected)throw Error('Invalid selection accepted');
          s=await invoke<InstallationStatus>('prepare_installation',{game,explorer});setStatus(s);await frame();
          const before=s.selection.game;
          const cancelled=invoke('prepare_installation',{game,explorer}).then(()=>false,()=>true);
          await frame();await invoke('cancel_preparation');if(!await cancelled)throw Error('Cancellation did not reject preparation');
          s=await invoke<InstallationStatus>('installation_status');if(!s.ready||s.selection.game!==before)throw Error('Cancellation lost active installation');
        }
        const c=await invoke<Catalog>('catalog');if(c.files.length!==1873||c.errors.length)throw Error('Prepared catalog failed');
        if(!document.querySelector('.installation-path'))throw Error('Selected installation control missing');
        const galleryButton=Array.from(document.querySelectorAll('button')).find(b=>b.textContent==='Background gallery');
        if(!galleryButton)throw Error('Gallery button missing');galleryButton.click();
        let thumbnailVerified=false;
        for(let attempt=0;attempt<75&&!thumbnailVerified;attempt++){
          await frame();
          for(const image of Array.from(document.querySelectorAll<HTMLImageElement>('.gallery-thumbnail img'))){
            if(!image.complete||!image.naturalWidth)continue;
            const canvas=document.createElement('canvas');canvas.width=image.naturalWidth;canvas.height=image.naturalHeight;
            const context=canvas.getContext('2d')!;context.drawImage(image,0,0);
            if(context.getImageData(0,0,canvas.width,canvas.height).data.some((v,i)=>i%4===3&&v>0)){thumbnailVerified=true;break}
          }
        }
        if(!thumbnailVerified)throw Error('Prepared background gallery produced no visible rendered thumbnails');
        (document.querySelector('.installation-path') as HTMLButtonElement).click();await frame();
        const gameField=document.querySelector('[aria-label="Game directory"]') as HTMLInputElement|null;
        const explorerField=document.querySelector('[aria-label="RDA Explorer directory"]') as HTMLInputElement|null;
        if(gameField?.value!==s.selection.game||explorerField?.value!==s.selection.explorer)throw Error('Settings did not retain selected folders');
        await invoke('setup_smoke_complete',{success:true,message:'Prepared catalog, directory settings and rendered gallery thumbnails passed.'});
      }catch(e){setError(String(e));if(setupSmoke)await invoke('setup_smoke_complete',{success:false,message:String(e)})}
    })();
  },[]);
  if(smoke)return <App/>;
  if(!status)return <div className="installation-setup"><h1>Anno GUI Studio</h1><p role="status">{error||'Opening…'}</p>{error&&<button onClick={()=>{setError('');void invoke<InstallationStatus>('installation_status').then(setStatus).catch(e=>setError(String(e)))}}>Retry</button>}</div>;
  return <>{status.ready&&<App key={generation} installation={status} onSetup={()=>setSettings(true)}/ >}{(!status.ready||settings)&&<InstallationSetup initial={status} onClose={status.ready?()=>setSettings(false):undefined} onReady={s=>{setStatus(s);setSettings(false);setGeneration(g=>g+1)}}/>}</>;
}
createRoot(document.getElementById('root')!).render(<Studio/>);
