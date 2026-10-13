//! Static unit-scale progress content from 00DB9C50 / 00DB9EF0.
use crate::{editor::Widget,layout::Preview};
use serde::{Deserialize,Serialize};
use std::{collections::BTreeMap,path::Path};
#[derive(Clone,Debug,Default,Deserialize,Serialize)]
pub struct Input {pub current:Option<i32>,pub maximum:Option<i32>}
#[derive(Serialize)]pub struct Layer {pub id:String,pub role:String,pub destination:[i32;4],pub clip:[i32;4],pub background:Option<crate::background::Background>,pub color:Option<u32>}
#[derive(Serialize)]pub struct Render {pub layers:Vec<Layer>,pub diagnostics:Vec<String>}
fn number(w:&Widget,key:&str,default:i32)->i32{w.properties.iter().find(|p|p.key==key).map(|p|if p.value.is_empty(){0}else{p.value.parse().unwrap_or(default)}).unwrap_or(default)}
pub fn fill(client:[i32;4],current:i32,maximum:i32,vertical:bool,reverse:bool)->Option<[i32;4]>{
    if maximum<=1{return None}
    let ratio=(current as f32/maximum as f32).clamp(0.0,1.0);let(w,h)=(client[2]-client[0],client[3]-client[1]);
    let mut r=[0,0,w,h];
    if vertical {if reverse{r[3]=(h as f32*ratio)as i32}else{r[1]=(h as f32*(1.0-ratio))as i32}}
    else if reverse{r[0]=(w as f32*(1.0-ratio))as i32}else{r[2]=(w as f32*ratio)as i32}
    for i in 0..4{r[i]+=client[i%2]}Some(r)
}
pub fn names(xml:&str)->Result<Vec<String>,String>{
    let doc=roxmltree::Document::parse(xml).map_err(|e|e.to_string())?;
    Ok(doc.root_element().children().find(|n|n.has_tag_name("BackgroundInfo")).ok_or("Missing BackgroundInfo")?.children().filter(|n|n.is_element()).map(|n|n.children().find(|c|c.has_tag_name("LayoutName")).and_then(|c|c.text()).unwrap_or("").trim().into()).collect())
}
pub fn render(repo:&Path,docs:&BTreeMap<String,String>,root:&Widget,preview:&Preview,inputs:&BTreeMap<String,Input>)->Render{
    render_candidate(repo,docs,root,preview,inputs,None)
}
pub fn render_candidate(repo:&Path,docs:&BTreeMap<String,String>,root:&Widget,preview:&Preview,inputs:&BTreeMap<String,Input>,archive:Option<&str>)->Render{
    let mut output=Render{layers:vec![],diagnostics:vec!["PROGRESSBAR fill and layer order use recovered unit-scale rules. Preview is a steady state; runtime animation, alpha/color transitions, scaling and pixel parity remain unverified.".into()]};
    fn visit(repo:&Path,docs:&BTreeMap<String,String>,w:&Widget,preview:&Preview,inputs:&BTreeMap<String,Input>,out:&mut Render,archive:Option<&str>){
        if w.kind=="PROGRESSBAR"{if let Some(r)=preview.rects.iter().find(|r|r.id==w.id){
            let input=inputs.get(&w.id).cloned().unwrap_or_default();
            let current=input.current.unwrap_or(number(w,"CURRENTVALUE",0));let maximum=input.maximum.unwrap_or(number(w,"MAXVALUE",10));
            if let Some(fill)=fill(r.client,current,maximum,number(w,"PROGRESSBAR_VERTICAL",0)!=0,number(w,"PROGRESSBAR_REVERSE",0)!=0){
                let clip=[fill[0].max(r.clip[0]),fill[1].max(r.clip[1]),fill[2].min(r.clip[2]),fill[3].min(r.clip[3])];
                let logical=w.properties.iter().find(|p|p.key=="BACKGROUND").map(|p|p.value.replace('\\',"/")).unwrap_or_default();
                if logical.is_empty(){let color=w.properties.iter().find(|p|p.key=="BARCOLOR").map(|p|p.value.parse::<u32>().unwrap_or(0)|0xff000000).unwrap_or(0xff0000ff);out.layers.push(Layer{id:w.id.clone(),role:"flat".into(),destination:fill,clip,background:None,color:Some(color)});}
                else {let result=(||->Result<(),String>{
                    let key=logical.split_once("/config/gui/").ok_or("Progress background path unresolved")?.1;
                    let xml=docs.get(key).ok_or("Progress background not in catalog")?;let names=names(xml)?;
                    if !names.iter().any(|n|n.eq_ignore_ascii_case("PB_Background")){return Err("PB_Background missing; client offsets depend on unresolved active-layout history".into())}
                    for(role,destination,clip)in [("PB_Bar",fill,clip),("PB_Overlay",[r.x,r.y,r.x+r.w,r.y+r.h],r.clip)]{
                        let Some(state)=names.iter().find(|n|n.eq_ignore_ascii_case(role))else{continue};
                        let(width,height)=(destination[2]-destination[0],destination[3]-destination[1]);if width<=0||height<=0||clip[2]<=clip[0]||clip[3]<=clip[1]{continue}
                        let background=crate::background::render_data_candidate(repo,xml,state,width,height,archive)?;
                        out.layers.push(Layer{id:w.id.clone(),role:role.into(),destination,clip,background:Some(background),color:None});
                    }Ok(())
                })();if let Err(e)=result{out.diagnostics.push(format!("{}: {e}",w.id));}}
            }
        }}
        for c in &w.children{visit(repo,docs,c,preview,inputs,out,archive)}
    }
    visit(repo,docs,root,preview,inputs,&mut output,archive);output
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn signed_clamping_float_truncation_and_direction_are_engine_rules(){
        let c=[10,20,111,71];assert_eq!(fill(c,1,3,false,false),Some([10,20,43,71]));
        assert_eq!(fill(c,1,3,false,true),Some([77,20,111,71]));
        assert_eq!(fill(c,1,3,true,false),Some([10,53,111,71]));
        assert_eq!(fill(c,1,3,true,true),Some([10,20,111,37]));
        assert_eq!(fill(c,-1,10,false,false),Some([10,20,10,71]));
        assert_eq!(fill(c,20,10,false,false),Some(c));assert_eq!(fill(c,1,1,false,false),None);
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn tiny_progress_art_allocates_fill_then_outer_overlay_without_source_edits(){
        let repo=crate::editor::discover_repo().unwrap();let key="backgrounds/final/general_progressbars/tiny_progressbar.bkg";
        let xml=std::fs::read_to_string(repo.join("data/addon/data1.rda/addondata/config/gui").join(key)).unwrap();
        let props=[("EXTENT/LEFT","20"),("EXTENT/TOP","30"),("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","20"),("CURRENTVALUE","50"),("MAXVALUE","100"),("BACKGROUND","addondata/config/gui/backgrounds/final/general_progressbars/tiny_progressbar.bkg")];
        let widget=Widget{id:"0".into(),kind:"PROGRESSBAR".into(),name:"fixture".into(),children:vec![],handlers:vec![],layers:vec![],warnings:vec![],properties:props.iter().map(|(k,v)|crate::editor::Property{key:(*k).into(),value:(*v).into(),source:"fixture".into(),local:true}).collect()};
        let offsets=crate::background::grid(&xml,"PB_Background",100,20).unwrap().client_offsets;
        let preview=crate::layout::preview(&widget,1,1280,1024,false,&BTreeMap::new(),&BTreeMap::from([("0".into(),offsets)]));
        let docs=BTreeMap::from([(key.into(),xml.clone())]);let rendered=render(&repo,&docs,&widget,&preview,&BTreeMap::new());
        assert_eq!(rendered.layers.len(),2,"{:?}",rendered.diagnostics);assert_eq!(rendered.layers[0].role,"PB_Bar");assert_eq!(rendered.layers[1].role,"PB_Overlay");
        let bar=rendered.layers[0].destination;assert_eq!(bar[2]-bar[0],(preview.rects[0].client[2]-preview.rects[0].client[0])/2);
        assert_eq!(rendered.layers[1].destination,[20,30,120,50]);assert!(!rendered.layers[0].background.as_ref().unwrap().textures.is_empty());
        assert_eq!(docs[key],xml);
        let empty=render(&repo,&docs,&widget,&preview,&BTreeMap::from([("0".into(),Input{current:Some(50),maximum:Some(1)})]));assert!(empty.layers.is_empty());
    }
}
