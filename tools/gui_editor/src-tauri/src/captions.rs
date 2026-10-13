//! Source GUID captions at unit scale. Cached game text is never a preview input.
use crate::{editor::Widget,layout,native_glyph,text_layout,localisation,native_text};
use serde::{Deserialize,Serialize};
use std::{path::Path,collections::{BTreeMap,BTreeSet}};
#[derive(Serialize)]pub struct Sprite{pub id:String,pub destination:[i32;4],pub clip:[i32;4],pub png:String}
#[derive(Clone,Deserialize,Serialize)]#[serde(deny_unknown_fields)]pub struct TextRefresh{pub client_width:i32,#[serde(default)]pub fit_wrapped_width:bool}
#[derive(Default,Deserialize,Serialize)]#[serde(deny_unknown_fields)]pub struct Runtime{pub current_city_name:Option<String>,#[serde(default)]pub text_refresh:BTreeMap<String,TextRefresh>,#[serde(default)]pub layout:layout::Scenario}
impl Runtime{pub fn validate(&self)->Result<(),String>{self.layout.validate()?;if self.text_refresh.len()>512{return Err("Text refresh scenario exceeds512 widgets".into())}for input in self.text_refresh.values(){if !(1..=8192).contains(&input.client_width){return Err("Pre-refresh client width must be1..8192".into())}}Ok(())}}
#[derive(Serialize)]pub struct Caption{pub id:String,pub guid:i32,pub source_text:String,pub sources:Vec<String>,pub parts:Vec<text_layout::Part>,pub origin:[i32;2],pub scenario_current_city_name:Option<String>}
#[derive(Serialize)]pub struct Preview{pub captions:Vec<Caption>,pub sprites:Vec<Sprite>,pub diagnostics:Vec<String>}
fn property<'a>(node:&'a Widget,key:&str)->Option<&'a str>{node.properties.iter().find(|p|p.key==key).map(|p|p.value.as_str())}
fn integer(node:&Widget,key:&str,default:i32)->Result<i32,String>{match property(node,key){None=>Ok(default),Some("")=>Ok(0),Some(v)=>v.parse().map_err(|_|format!("Invalid {key}"))}}
fn font(repo:&Path,language:&str,name:&str)->Result<(text_layout::Font,native_glyph::Font),String>{
    let candidates=localisation::inspect(repo,language,-1,name)?.fonts;
    let values:BTreeSet<_>=candidates.iter().map(|c|c.value.clone()).collect();
    if values.len()!=1{return Err(format!("Font source missing or ambiguous: {name}"))}
    let xml=values.into_iter().next().unwrap();let spec=native_text::font_spec(&xml)?;
    let doc=roxmltree::Document::parse(&xml).map_err(|e|e.to_string())?;
    let default_color=doc.root_element().children().find(|n|n.has_tag_name("DefaultColor")).and_then(|n|n.text()).and_then(|v|v.trim().parse::<u32>().ok());
    let mut resolved=None;for candidate in &candidates{let document=localisation::font_document(repo,language,name,&candidate.source)?;let current=native_glyph::font_from_document(&document,name,[1.0,1.0])?;if let Some(previous)=&resolved{if serde_json::to_string(previous).map_err(|e|e.to_string())?!=serde_json::to_string(&current).map_err(|e|e.to_string())?{return Err(format!("Font document context ambiguous: {name}"))}}resolved=Some(current);}
    Ok((text_layout::Font{spec,default_color},resolved.ok_or("Font document missing")?))
}
fn source_color(node:&Widget,key:&str)->Result<u32,String>{
    let value=property(node,key).ok_or_else(||format!("Active caption color {key} missing; retained/default state unresolved"))?;
    let v=if value.is_empty(){0}else{value.parse::<i64>().map_err(|_|format!("Invalid {key}"))?};
    if !(-2147483648..=4294967295).contains(&v){return Err(format!("{key} outside verified integer range"))}Ok(v as u32|0xff000000)
}
fn caption_color(node:&Widget,input:&crate::button_state::Input,enabled_chain:bool)->Result<Option<u32>,String>{
    if node.kind=="LABEL"{return property(node,"TEXTSETTING/TEXTCOLOR").map(|_|source_color(node,"TEXTSETTING/TEXTCOLOR")).transpose()}
    if node.kind!="BUTTON"{return Err(format!("{} caption renderer not recovered",node.kind))}
    // D3FBA0 explicitly clears the color flag even when TEXTSETTING has a color.
    if property(node,"TEXTCOLOR_ENABLE").is_none(){return Err("Button TEXTCOLOR_ENABLE constructor default unresolved".into())}
    if integer(node,"TEXTCOLOR_ENABLE",0)?==0{return Ok(None)}
    if input.force_checked_hover{return Err("Forced checked-hover caption retains previous color; history input required".into())}
    let key=if !enabled_chain||input.force_disabled{"DISABLEDTEXTCOLOR"}else if input.checked||input.pressed{"PRESSEDBUTTONTEXTCOLOR"}else if input.hovered&&!input.suppress_hover{"MOUSEOVERTEXTCOLOR"}else{"TEXTSETTING/TEXTCOLOR"};
    source_color(node,key).map(Some)
}
struct Prepared{guid:i32,text:String,sources:Vec<String>,layout:text_layout::ResultLayout,raster_fonts:BTreeMap<String,native_glyph::Font>}
fn prepare(repo:&Path,language:&str,node:&Widget,client_width:i32,input:&crate::button_state::Input,enabled_chain:bool,runtime:&Runtime)->Result<Prepared,String>{
    let guid=integer(node,"TEXTSETTING/TEXTGUID",-1)?;
    let inspected=localisation::inspect(repo,language,guid,"")?;
    let values:BTreeSet<_>=inspected.texts.iter().map(|c|c.value.clone()).collect();if values.len()!=1{return Err("GUID source missing or ambiguous".into())}
    let text=values.into_iter().next().unwrap();
    let name=property(node,"TEXTSETTING/FONT").filter(|s|!s.is_empty()).ok_or("Font fallback not established for this source")?;
    let mut names=BTreeSet::from([name.to_string()]);
    // FONT is the only tag that can introduce another font. Escaped/compound tags
    // remain the tokenizer's responsibility; this collects candidates only.
    for source in std::iter::once(text.as_str()).chain(runtime.current_city_name.as_deref()){for suffix in source.split("[FONT ").skip(1){if let Some(end)=suffix.find([']',';']){names.insert(suffix[..end].to_string());}}}
    let mut fonts=vec![];let mut raster_fonts=BTreeMap::new();for name in names{let(layout,raster)=font(repo,language,&name)?;raster_fonts.insert(name, raster);fonts.push(layout);}
    let mut flags=2;let color=caption_color(node,input,enabled_chain)?;if color.is_some(){flags|=4;}
    if node.kind=="BUTTON"&&!enabled_chain{flags|=0x2000;}
    match integer(node,"TEXTSETTING/ALIGNMENT_HORIZONTAL",0)?{1=>flags|=0x100,2=>flags|=0x800,_=>{}}
    if integer(node,"TEXTSETTING/ENABLE_LINEBREAK",0)?!=0{flags|=0x80;}
    if integer(node,"TEXTSETTING/ENABLE_DOTS",0)?!=0{return Err("Ellipsis layout not implemented".into())}
    let constrained=flags&(0x80|0x4900)!=0;if constrained{flags|=0x40;}
    let retained=runtime.text_refresh.get(&node.id);if let Some(input)=retained{if !(1..=8192).contains(&input.client_width){return Err("Pre-refresh client width must be1..8192".into())}}
    let width=if constrained{retained.map(|i|i.client_width).unwrap_or(client_width).min(integer(node,"TEXTSETTING/PARENTWINDOWMAXWIDTH",i32::MAX)?)}else{i32::MAX};
    let layout=text_layout::layout_bounds_scenario(text_layout::Input{text:&text,font:name,fonts:&fonts,scale:[1.0,1.0],flags,width,color},runtime.current_city_name.as_deref())?;
    Ok(Prepared{guid,text,sources:inspected.texts.iter().map(|c|format!("{} SHA256 {}",c.source,c.sha256)).collect(),layout,raster_fonts})
}
fn render_one(repo:&Path,language:&str,node:&Widget,rect:&layout::Rect,input:&crate::button_state::Input,enabled_chain:bool,runtime:&Runtime)->Result<(Caption,Vec<Sprite>),String>{
    let Prepared{guid,text,sources,layout,raster_fonts}=prepare(repo,language,node,rect.client[2]-rect.client[0],input,enabled_chain,runtime)?;
    let parts=layout.parts;let vertical=integer(node,"TEXTSETTING/ALIGNMENT_VERTICAL",0)?;
    let gap=(rect.client[3]-rect.client[1]-layout.size[1]).max(0);
    let origin=[rect.client[0],rect.client[1]+match vertical{1=>gap/2,2=>gap,_=>0}];
    let mut sprites=vec![];for part in &parts{
        let name=part.font.as_ref().ok_or("Inline icon caption pixels need separate tint/clip verification")?;
        if part.text.is_empty(){continue}
        let color=((((part.color>>24)as f32*rect.alpha)as u32).min(255)<<24)|(part.color&0xffffff);
        let raster=native_glyph::render_run(&native_glyph::RasterInput{font:raster_fonts.get(name).ok_or("Raster font missing")?.clone(),text:part.text.clone(),color})?;
        sprites.push(Sprite{id:node.id.clone(),destination:[origin[0]+part.offset[0]+raster.origin[0],origin[1]+part.offset[1]+raster.origin[1],raster.size[0]as i32,raster.size[1]as i32],clip:rect.clip,png:raster.png});
    }
    Ok((Caption{id:node.id.clone(),guid,source_text:text,sources,parts,origin,scenario_current_city_name:runtime.current_city_name.clone()},sprites))
}
fn enabled_tree(node:&Widget,parent:bool,buttons:&BTreeMap<String,crate::button_state::Input>,out:&mut BTreeMap<String,bool>){let source=integer(node,"DISABLE",0).map(|v|v==0).unwrap_or(false);let enabled=parent&&buttons.get(&node.id).and_then(|v|v.enabled).unwrap_or(source);out.insert(node.id.clone(),enabled);for child in &node.children{enabled_tree(child,enabled,buttons,out)}}
pub fn intrinsic_sizes(repo:&Path,language:&str,tree:&Widget,buttons:&BTreeMap<String,crate::button_state::Input>,runtime:&Runtime,offsets:&BTreeMap<String,[i32;4]>)->(BTreeMap<String,layout::TextSize>,Vec<String>){
    let mut enabled=BTreeMap::new();enabled_tree(tree,true,buttons,&mut enabled);
    let mut sizes=BTreeMap::new();let mut diagnostics=vec![];
    fn walk(repo:&Path,language:&str,node:&Widget,buttons:&BTreeMap<String,crate::button_state::Input>,runtime:&Runtime,offsets:&BTreeMap<String,[i32;4]>,enabled:&BTreeMap<String,bool>,sizes:&mut BTreeMap<String,layout::TextSize>,diagnostics:&mut Vec<String>){
        if integer(node,"TEXTSETTING/RESIZEPARENTWINDOW",0).is_ok_and(|v|v!=0){
            let measured:Result<layout::TextSize,String>=(||{
                if node.kind!="LABEL"{return Err("Text resizing currently requires LABEL; other owner lifecycles unverified".into())}
                let wrapped=integer(node,"TEXTSETTING/ENABLE_LINEBREAK",0)?!=0;
                let constrained=integer(node,"TEXTSETTING/ALIGNMENT_HORIZONTAL",0)?!=0||wrapped||integer(node,"TEXTSETTING/ENABLE_DOTS",0)?!=0;
                let retained=runtime.text_refresh.get(&node.id);
                if constrained&&retained.is_none(){return Err("Constrained text resizing requires an explicit pre-refresh client width and refresh mode".into())}
                if property(node,"BACKGROUND").is_some_and(|v|!v.is_empty())&&!offsets.contains_key(&node.id){return Err("Text resize needs resolved active background client offsets".into())}
                let input=buttons.get(&node.id).cloned().unwrap_or_default();
                let prepared=prepare(repo,language,node,i32::MAX,&input,*enabled.get(&node.id).unwrap_or(&false),runtime)?;
                let mut client=prepared.layout.size;if wrapped{if let Some(input)=retained{if !input.fit_wrapped_width{client[0]=input.client_width;}}}
                Ok(layout::TextSize{client,far_anchor:integer(node,"TEXTSETTING/ALIGNMENT_VERTICAL",0)?==2,pre_refresh_width:if constrained{retained.map(|i|i.client_width)}else{None}})
            })();
            match measured{Ok(size)=>{sizes.insert(node.id.clone(),size);},Err(error)=>diagnostics.push(format!("{}: text resize unresolved: {error}",node.name))}
        }
        for child in &node.children{walk(repo,language,child,buttons,runtime,offsets,enabled,sizes,diagnostics)}
    }
    walk(repo,language,tree,buttons,runtime,offsets,&enabled,&mut sizes,&mut diagnostics);(sizes,diagnostics)
}
pub fn preview(repo:&Path,language:&str,tree:&Widget,layout:&layout::Preview)->Preview{
    preview_scenario(repo,language,tree,layout,&BTreeMap::new())
}
pub fn preview_scenario(repo:&Path,language:&str,tree:&Widget,layout:&layout::Preview,buttons:&BTreeMap<String,crate::button_state::Input>)->Preview{
    preview_runtime(repo,language,tree,layout,buttons,&Runtime::default())
}
pub fn preview_runtime(repo:&Path,language:&str,tree:&Widget,layout:&layout::Preview,buttons:&BTreeMap<String,crate::button_state::Input>,runtime:&Runtime)->Preview{
    let mut enabled=BTreeMap::new();enabled_tree(tree,true,buttons,&mut enabled);
    fn lookup<'a>(node:&'a Widget,id:&str)->Option<&'a Widget>{if node.id==id{Some(node)}else{node.children.iter().find_map(|child|lookup(child,id))}}
    let mut result=Preview{captions:vec![],sprites:vec![],diagnostics:vec!["Source GUID captions at unit scale. CURRENT_CITY_NAME uses an explicit resolved scenario and rescans inserted markup. Source LABEL resize precedes widget layout; constrained captions require explicit pre-refresh width/mode. Page0 metadata verified; higher Unicode pages, other runtime tags, generated text, automatic refresh history, inline icon pixels and full source/render parity remain unsupported or unverified.".into()]};
    for rect in &layout.rects{let Some(node)=lookup(tree,&rect.id)else{continue};if property(node,"TEXTSETTING/TEXTGUID").is_none(){continue}if integer(node,"TEXTSETTING/TEXTGUID",-1).is_ok_and(|guid|guid<=0){continue}
        if node.kind=="BUTTON"{if property(node,"BUTTON_TEXT_ENABLE").is_none(){result.diagnostics.push(format!("{}: Button caption enable default unresolved",node.name));continue}if integer(node,"BUTTON_TEXT_ENABLE",0).is_ok_and(|v|v==0){continue}}
        let input=buttons.get(&node.id).cloned().unwrap_or_default();
        if integer(node,"TEXTSETTING/RESIZEPARENTWINDOW",0).is_ok_and(|v|v!=0)&&!layout.text_sizes.contains_key(&node.id){result.diagnostics.push(format!("{}: owner resize unresolved; caption omitted",node.name));continue}
        match render_one(repo,language,node,rect,&input,*enabled.get(&node.id).unwrap_or(&false),runtime){Ok((caption,sprites))=>{result.captions.push(caption);result.sprites.extend(sprites)},Err(error)=>result.diagnostics.push(format!("{}: {error}",node.name))}
    }result
}
#[cfg(all(test,windows))]mod tests{
    use super::*;
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn constrained_original_label_preserves_width_and_cached_parts_after_fit(){
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/load_text_frame.dlg").unwrap();let node=&view.tree.as_ref().unwrap().children[0];
        assert_eq!(integer(node,"TEXTSETTING/TEXTGUID",-1).unwrap(),160014);assert_eq!(integer(node,"EXTENT/WIDTH",0).unwrap(),296);
        let key=property(node,"BACKGROUND").unwrap().split("/config/gui/").nth(1).unwrap();let background=editor.background(key,"EmptyLayout",296,60).unwrap();assert_eq!(background.client_offsets,[8;4]);
        // Independent authored extent/background determine pre-refresh width.
        let initial=296-background.client_offsets[0]-background.client_offsets[2];
        let mut runtime=Runtime{text_refresh:BTreeMap::from([(node.id.clone(),TextRefresh{client_width:initial,fit_wrapped_width:false})]),..Default::default()};
        let geometry=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),"eng",&runtime).unwrap();
        assert_eq!(geometry.text_sizes[&node.id].client,[280,34]);let rect=geometry.rects.iter().find(|r|r.id==node.id).unwrap();
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/text-owner-load-frame-live.json")).unwrap();assert_eq!(serde_json::json!(rect.raw_relative),fixture["widget"]["Relative"]);
        let(caption,_)=render_one(&repo,"eng",node,rect,&Default::default(),true,&runtime).unwrap();
        let captured:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/text-inline-live-1440x1080.json")).unwrap();let block=captured["blocks"].as_array().unwrap().iter().find(|b|b["Guid"]==160014).unwrap();
        let actual:Vec<_>=captured["parts"].as_array().unwrap().iter().filter(|p|p["Widget"]==block["Widget"]&&p["ProcessedId"]==block["ProcessedId"]).collect();assert_eq!(caption.parts.len(),actual.len());for(p,a)in caption.parts.iter().zip(actual){assert_eq!(serde_json::json!(p.text),a["Text"]);assert_eq!(serde_json::json!(p.size),a["Size"]);assert_eq!(serde_json::json!(p.offset),a["Offset"]);assert_eq!(serde_json::json!(p.line_rect),a["LineRect"]);assert_eq!(serde_json::json!(p.color),a["Color"]);}
        runtime.text_refresh.get_mut(&node.id).unwrap().fit_wrapped_width=true;
        let fitted=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),"eng",&runtime).unwrap();assert_eq!(fitted.text_sizes[&node.id].client,[259,34]);
        let fitted_rect=fitted.rects.iter().find(|r|r.id==node.id).unwrap();assert_eq!(fitted_rect.raw_relative,[36,30,311,80]);let(fitted_caption,_)=render_one(&repo,"eng",node,fitted_rect,&Default::default(),true,&runtime).unwrap();assert_eq!(caption.parts,fitted_caption.parts,"Resize must retain text's pre-refresh wrap/center width, not reflow at its new owner width");
        let mut limited=node.clone();limited.properties.push(crate::editor::Property{key:"TEXTSETTING/PARENTWINDOWMAXWIDTH".into(),value:"250".into(),source:"scenario test".into(),local:true});runtime.text_refresh.get_mut(&node.id).unwrap().fit_wrapped_width=false;
        let offsets=BTreeMap::from([(node.id.clone(),[8;4])]);let(sizes,diagnostics)=intrinsic_sizes(&repo,"eng",&limited,&BTreeMap::new(),&runtime,&offsets);assert!(diagnostics.is_empty(),"{diagnostics:?}");assert_eq!(sizes[&node.id].client[0],280,"Wrap preserves pre-clamp owner width");
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn refresh_scenarios_reject_bad_width_and_unknown_widget(){
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/load_text_frame.dlg").unwrap();
        let mut runtime=Runtime{text_refresh:BTreeMap::from([("missing".into(),TextRefresh{client_width:280,fit_wrapped_width:false})]),..Default::default()};assert!(editor.preview_layout_text_scenario(view.revision,1280,1024,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),"eng",&runtime).err().unwrap().contains("not in this definition"));
        runtime.text_refresh.get_mut("missing").unwrap().client_width=0;assert!(runtime.validate().unwrap_err().contains("1..8192"));
        assert!(serde_json::from_str::<Runtime>(r#"{"text_refresh":{"0":{"client_width":280,"unknown":true}}}"#).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn original_island_caption_resizes_before_layout_and_matches_relative_capture(){
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/infobar_island_info.dlg").unwrap();
        fn find(node:&Widget)->Option<&Widget>{if node.name=="ISLAND_NAME"{Some(node)}else{node.children.iter().find_map(find)}}
        let node=find(view.tree.as_ref().unwrap()).unwrap();
        assert_eq!(property(node,"EXTENT/WIDTH"),Some("96"));
        let runtime=Runtime{current_city_name:Some("First Island".into()),..Default::default()};
        let geometry=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),"eng",&runtime).unwrap();
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/text-owner-first-island-live.json")).unwrap();
        let rect=geometry.rects.iter().find(|r|r.id==node.id).unwrap();
        assert_eq!(serde_json::json!(rect.raw_relative),fixture["widget"]["Relative"]);
        assert_eq!(geometry.text_sizes[&node.id].client,[90,22]);
        let preview=preview_runtime(&repo,"eng",view.tree.as_ref().unwrap(),&geometry,&BTreeMap::new(),&runtime);
        let caption=preview.captions.iter().find(|c|c.id==node.id).unwrap();assert_eq!(caption.parts.iter().map(|p|p.text.as_str()).collect::<String>(),"First Island");assert!(preview.sprites.iter().any(|s|s.id==node.id));
        // No captured ancestor coordinates enter the prediction. Absolute
        // placement still depends on runtime sequence/controller behavior.
        let unresolved=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),"eng",&Runtime::default()).unwrap();
        assert!(!unresolved.text_sizes.contains_key(&node.id));assert!(unresolved.diagnostics.iter().any(|d|d.contains("Missing CURRENT_CITY_NAME")));
        let mut constrained=node.clone();constrained.properties.iter_mut().find(|p|p.key=="TEXTSETTING/ENABLE_LINEBREAK").unwrap().value="1".into();
        let(sizes,diagnostics)=intrinsic_sizes(&repo,"eng",&constrained,&BTreeMap::new(),&runtime,&BTreeMap::new());assert!(sizes.is_empty());assert!(diagnostics.iter().any(|d|d.contains("pre-refresh")));
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn original_city_name_tag_and_explicit_scenario_match_captured_local_parts(){
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/infobar_island_info.dlg").unwrap();
        fn find(node:&Widget)->Option<&Widget>{if property(node,"TEXTSETTING/TEXTGUID")==Some("198142"){Some(node)}else{node.children.iter().find_map(find)}}
        let node=find(view.tree.as_ref().unwrap()).unwrap();let text=localisation::inspect(&repo,"eng",198142,"").unwrap().texts[0].value.clone();assert_eq!(text,"[CURRENT_CITY_NAME]");
        let(name,_)=font(&repo,"eng",property(node,"TEXTSETTING/FONT").unwrap()).unwrap();let fonts=[name];
        let scenario:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/glyph-headlines-input.json")).unwrap();
        let parts=text_layout::layout_scenario(text_layout::Input{text:&text,font:property(node,"TEXTSETTING/FONT").unwrap(),fonts:&fonts,scale:[1.0,1.0],flags:6,width:i32::MAX,color:Some(source_color(node,"TEXTSETTING/TEXTCOLOR").unwrap())},scenario["text"].as_str()).unwrap();
        let fixture:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/text-inline-live-1440x1080.json")).unwrap();let block=fixture["blocks"].as_array().unwrap().iter().find(|v|v["Guid"]==198142).unwrap();let actual:Vec<_>=fixture["parts"].as_array().unwrap().iter().filter(|v|v["Widget"]==block["Widget"]&&v["ProcessedId"]==block["ProcessedId"]).collect();assert_eq!(parts.len(),actual.len());
        for(p,a)in parts.iter().zip(actual){assert_eq!(serde_json::json!(p.text),a["Text"]);assert_eq!(serde_json::json!(p.size),a["Size"]);assert_eq!(serde_json::json!(p.offset),a["Offset"]);assert_eq!(serde_json::json!(p.line_rect),a["LineRect"]);assert_eq!(serde_json::json!(p.color),a["Color"]);}
        // This is a local-part comparison with an explicit name; relative owner resizing
        // has its own fixture and sticky-island live name selection is separate.
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn original_button_caption_uses_scenario_colors_and_explicit_enable(){
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/exit_game_menu.dlg").unwrap();
        fn find(node:&Widget)->Option<&Widget>{if node.kind=="BUTTON"&&integer(node,"BUTTON_TEXT_ENABLE",0)==Ok(1)&&integer(node,"TEXTSETTING/TEXTGUID",-1).is_ok_and(|v|v>0){Some(node)}else{node.children.iter().find_map(find)}}
        let node=find(view.tree.as_ref().unwrap()).expect("Original caption button");
        let mut input=crate::button_state::Input::default();
        for (hovered,pressed,checked,enabled,key) in [(false,false,false,true,"TEXTSETTING/TEXTCOLOR"),(true,false,false,true,"MOUSEOVERTEXTCOLOR"),(false,true,false,true,"PRESSEDBUTTONTEXTCOLOR"),(false,false,true,true,"PRESSEDBUTTONTEXTCOLOR"),(true,true,true,false,"DISABLEDTEXTCOLOR")]{input.hovered=hovered;input.pressed=pressed;input.checked=checked;assert_eq!(caption_color(node,&input,enabled).unwrap(),Some(source_color(node,key).unwrap()));}
        input.force_checked_hover=true;assert!(caption_color(node,&input,true).unwrap_err().contains("history"));
        let mut changed=node.clone();changed.properties.iter_mut().find(|p|p.key=="TEXTCOLOR_ENABLE").unwrap().value="0".into();assert_eq!(caption_color(&changed,&input,true).unwrap(),None);
        input=Default::default();changed=node.clone();changed.properties.retain(|p|p.key!="MOUSEOVERTEXTCOLOR");input.hovered=true;assert!(caption_color(&changed,&input,true).unwrap_err().contains("missing"));
        let geometry=editor.preview_layout(view.revision,1440,1080,true,&BTreeMap::new()).unwrap();let rect=geometry.rects.iter().find(|r|r.id==node.id).unwrap();
        let(caption,sprites)=render_one(&repo,"eng",node,rect,&Default::default(),true,&Runtime::default()).unwrap();assert!(!sprites.is_empty());assert!(caption.parts.iter().all(|part|part.color==source_color(node,"TEXTSETTING/TEXTCOLOR").unwrap()));
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn source_caption_parts_match_independent_guid_capture(){
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/save_dialog.dlg").unwrap();
        fn find(node:&Widget)->Option<&Widget>{if property(node,"TEXTSETTING/TEXTGUID")==Some("122002"){Some(node)}else{node.children.iter().find_map(find)}}
        let node=find(view.tree.as_ref().unwrap()).unwrap();
        let geometry=editor.preview_layout(view.revision,1440,1080,false,&BTreeMap::new()).unwrap();let rect=geometry.rects.iter().find(|r|r.id==node.id).unwrap();
        let (caption,sprites)=render_one(&repo,"eng",node,rect,&Default::default(),true,&Runtime::default()).unwrap();assert!(!sprites.is_empty());assert!(!caption.sources.is_empty());
        let capture:serde_json::Value=serde_json::from_str(include_str!("../../fixtures/text-inline-live-1440x1080.json")).unwrap();
        // The original GUID string is independent of these comparison outputs.
        let block=capture["blocks"].as_array().unwrap().iter().find(|b|b["Guid"]==122002).unwrap();
        let parts:Vec<_>=capture["parts"].as_array().unwrap().iter().filter(|p|p["Widget"]==block["Widget"]&&p["ProcessedId"]==block["ProcessedId"]).collect();
        assert_eq!(caption.parts.len(),parts.len());for(predicted,actual)in caption.parts.iter().zip(parts){assert_eq!(predicted.text,actual["Text"].as_str().unwrap());assert_eq!(serde_json::json!(predicted.size),actual["Size"]);assert_eq!(serde_json::json!(predicted.offset),actual["Offset"]);assert_eq!(serde_json::json!(predicted.color),actual["Color"]);}
    }
}

