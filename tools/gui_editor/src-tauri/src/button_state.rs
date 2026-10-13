//! Static button background selection recovered from 00D3DA00 / 00D3E2C0.
//! Inputs are preview scenario data, never authored XML or live interaction.
use serde::{Deserialize,Serialize};
#[derive(Clone,Debug,Default,Deserialize,Serialize)]
#[serde(default)]
pub struct Input {pub hovered:bool,pub pressed:bool,pub checked:bool,pub enabled:Option<bool>,pub suppress_hover:bool,pub force_disabled:bool,pub force_checked_hover:bool}
#[derive(Clone,Debug,Serialize)]
pub struct Selection {pub requested:String,pub selected:String,pub index:usize,pub fallback:bool}
pub fn requested(input:&Input,enabled_chain:bool)->String {
    if input.force_checked_hover{return "Checked+MouseOver".into()}
    let base=if input.checked{"Checked"}else{"Normal"};
    let hover=input.hovered&&!input.suppress_hover;
    let suffix=if !enabled_chain||input.force_disabled{"+Disabled"}else if hover&&input.pressed{"+MouseButtonDown"}else if hover||input.pressed{"+MouseOver"}else{""};
    format!("{base}{suffix}")
}
pub fn select(xml:&str,input:&Input,enabled_chain:bool)->Result<Selection,String>{
    let document=roxmltree::Document::parse(xml).map_err(|e|e.to_string())?;
    let layouts=document.root_element().children().find(|n|n.has_tag_name("BackgroundInfo")).ok_or("Missing BackgroundInfo")?;
    let names:Vec<String>=layouts.children().filter(|n|n.is_element()).map(|n|n.children().find(|c|c.has_tag_name("LayoutName")).and_then(|c|c.text()).unwrap_or("").trim().into()).collect();
    select_names(&names,input,enabled_chain)
}
pub fn select_names(names:&[String],input:&Input,enabled_chain:bool)->Result<Selection,String>{
    if names.is_empty(){return Err("Button background has no layouts".into())}
    // The engine stores relative layout indices in bytes. Reject unusual larger
    // sets until subset offsets and index truncation are represented explicitly.
    if names.len()>256{return Err("Button layout set exceeds verified byte-index scope".into())}
    let requested=requested(input,enabled_chain);
    let found=names.iter().position(|n|n.eq_ignore_ascii_case(&requested));
    let index=found.unwrap_or(0);
    Ok(Selection{requested,selected:names[index].clone(),index,fallback:found.is_none()})
}
#[derive(Deserialize)]pub struct Live {pub widget:u32,pub input:Input,pub enabled_chain:bool,pub layouts:Vec<String>,pub actual_index:usize}
#[derive(Deserialize)]pub struct Snapshot {pub schema:u32,pub captured_utc:String,pub binary_sha256:String,pub buttons:Vec<Live>}
#[derive(Serialize)]pub struct Row {pub widget:u32,pub selection:Selection,pub actual_index:usize,pub status:String}
#[derive(Serialize)]pub struct Report {pub captured_utc:String,pub binary_sha256:String,pub counts:std::collections::BTreeMap<String,usize>,pub rows:Vec<Row>,pub diagnostics:Vec<String>}
pub fn compare(json:&str)->Result<Report,String>{
    if json.len()>32_000_000{return Err("Button capture exceeds 32MB".into())}
    let snapshot:Snapshot=serde_json::from_str(json).map_err(|e|e.to_string())?;
    if snapshot.schema!=1{return Err("Use capture-button-reference.ps1 schema1".into())}
    let mut report=Report{captured_utc:snapshot.captured_utc,binary_sha256:snapshot.binary_sha256,counts:Default::default(),rows:vec![],diagnostics:vec!["Visible-chain cached button layout indices only. Names and scenario states are live inputs; the active layout index is a comparison output. Non-atomic captures and stale refreshes may mismatch. Pixel/color/icon/text/focus-overlay parity remains separate.".into()]};
    for button in snapshot.buttons{
        let selection=select_names(&button.layouts,&button.input,button.enabled_chain)?;
        let status=if selection.index==button.actual_index{"exact"}else{"mismatch"};
        *report.counts.entry(status.into()).or_default()+=1;
        report.rows.push(Row{widget:button.widget,selection,actual_index:button.actual_index,status:status.into()});
    }
    Ok(report)
}
#[cfg(test)]mod tests{
    use super::*;
    #[test]fn pressed_outside_hover_and_disabled_checked_use_engine_branches(){
        let mut input=Input{pressed:true,..Input::default()};
        assert_eq!(requested(&input,true),"Normal+MouseOver");
        input.hovered=true;assert_eq!(requested(&input,true),"Normal+MouseButtonDown");
        input.checked=true;assert_eq!(requested(&input,false),"Checked+Disabled");
        input.pressed=false;assert_eq!(requested(&input,true),"Checked+MouseOver");
        input.suppress_hover=true;assert_eq!(requested(&input,true),"Checked");
        input.force_disabled=true;assert_eq!(requested(&input,true),"Checked+Disabled");
        input.force_checked_hover=true;assert_eq!(requested(&input,false),"Checked+MouseOver");
    }
    #[test]fn missing_variant_uses_first_layout_not_named_normal(){
        let xml="<Background><BackgroundInfo><i><LayoutName>EmptyLayout</LayoutName></i><i><LayoutName>NORMAL</LayoutName></i><i><LayoutName>checked+disabled</LayoutName></i></BackgroundInfo></Background>";
        let normal=select(xml,&Input::default(),true).unwrap();assert_eq!(normal.index,1);assert!(!normal.fallback);
        let hover=select(xml,&Input{hovered:true,..Input::default()},true).unwrap();assert_eq!(hover.selected,"EmptyLayout");assert!(hover.fallback);
        let disabled=select(xml,&Input{checked:true,..Input::default()},false).unwrap();assert_eq!(disabled.index,2);
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn captured_visible_states_match_without_using_active_index_as_input(){
        let repo=crate::editor::discover_repo().unwrap();
        let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/button-states-live-1440x1080.json")).unwrap();
        let report=compare(&json).unwrap();assert_eq!(report.counts.get("exact"),Some(&53));assert!(!report.counts.contains_key("mismatch"));
        let mut changed:serde_json::Value=serde_json::from_str(&json).unwrap();changed["buttons"][0]["actual_index"]=serde_json::json!(9999);
        assert_eq!(compare(&changed.to_string()).unwrap().counts.get("mismatch"),Some(&1));
    }
}
