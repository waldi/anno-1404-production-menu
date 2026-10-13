//! Native-coordinate geometry comparisons. A match is evidence for one captured
//! scenario only; missing/hidden/ambiguous nodes are retained in the report.
use crate::{editor::Widget, layout::Preview};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct LiveWidget {
    address: u32, path_id: String, relative: [i32;4], absolute: [i32;4],
    visible: i32, flags: u32, children: Vec<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Screen { gui_size:[i32;2], scale:[f32;2] }
#[derive(Deserialize)]
pub struct Snapshot {
    schema:u32, captured_utc:String, binary_sha256:String,
    gui_screen:Screen, widgets:Vec<LiveWidget>,
}
#[derive(Serialize)]
pub struct Row {
    pub path:String, pub id:String, pub status:String,
    pub raw_delta:Option<[i32;4]>, pub absolute_delta:Option<[i32;4]>,
    pub preview_raw:Option<[i32;4]>, pub live_raw:Option<[i32;4]>,
    pub preview_absolute:Option<[i32;4]>, pub live_absolute:Option<[i32;4]>,
    pub diagnostics:Vec<String>,
}
#[derive(Serialize)]
pub struct Report {
    pub captured_utc:String, pub binary_sha256:String, pub viewport:[i32;2],
    pub counts:BTreeMap<String,usize>, pub rows:Vec<Row>,pub diagnostics:Vec<String>,
}
fn delta(a:[i32;4],b:[i32;4])->[i32;4] { [a[0]-b[0],a[1]-b[1],a[2]-b[2],a[3]-b[3]] }
#[derive(Serialize)]
pub struct Scenario {pub visibility:BTreeMap<String,bool>,pub diagnostics:Vec<String>}
pub fn scenario(json:&str,tree:&Widget,width:i32,height:i32)->Result<Scenario,String>{
    if json.len()>32_000_000{return Err("Snapshot exceeds 32 MB".into())}
    let s:Snapshot=serde_json::from_str(json).map_err(|e|e.to_string())?;
    if s.schema!=2||s.gui_screen.gui_size!=[width,height]||s.gui_screen.scale!=[1.0,1.0]{return Err("Use schema-2 snapshot with matching unit-scale GUI viewport".into())}
    let roots:Vec<_>=s.widgets.iter().filter(|w|w.path_id==tree.name).collect();
    if roots.len()!=1{return Err("Dialog root is missing or ambiguous".into())}
    let map:BTreeMap<_,_>=s.widgets.iter().map(|w|(w.address,w)).collect();
    let mut visibility=BTreeMap::new();let mut diagnostics=vec!["Only captured visibility is supplied. No captured extents are used as layout inputs, and source XML is untouched.".into()];
    fn walk(node:&Widget,live:&LiveWidget,map:&BTreeMap<u32,&LiveWidget>,values:&mut BTreeMap<String,bool>,diagnostics:&mut Vec<String>){
        values.insert(node.id.clone(),live.visible==1&&live.flags&0x400000==0);
        for child in &node.children {
            let matches:Vec<_>=live.children.iter().filter_map(|a|map.get(a).copied()).filter(|w|w.path_id==child.name).collect();
            if matches.len()==1{walk(child,matches[0],map,values,diagnostics)}else{diagnostics.push(format!("{} visibility unresolved: {} live matches",child.id,matches.len()));}
        }
    }
    walk(tree,roots[0],&map,&mut visibility,&mut diagnostics);
    Ok(Scenario{visibility,diagnostics})
}
pub fn compare(json:&str, tree:&Widget, preview:&Preview,width:i32,height:i32)->Result<Report,String> {
    if json.len()>32_000_000 {return Err("Snapshot exceeds 32 MB".into())}
    let snapshot:Snapshot=serde_json::from_str(json).map_err(|e|format!("Invalid schema-2 capture: {e}"))?;
    if snapshot.schema!=2 {return Err("Use a schema-2 capture-reference.ps1 snapshot".into())}
    if snapshot.gui_screen.gui_size!=[width,height] {return Err(format!("GUI viewport differs: snapshot {:?}, preview [{width},{height}]. No scaling is performed.",snapshot.gui_screen.gui_size))}
    if snapshot.gui_screen.scale!=[1.0,1.0] {return Err("Non-unit GUI scale is not implemented; comparison would be misleading".into())}
    let roots:Vec<_>=snapshot.widgets.iter().filter(|n|n.path_id==tree.name).collect();
    if roots.len()!=1 {return Err(format!("Root {} has {} live matches; choose an unambiguous dialog",tree.name,roots.len()))}
    let by_address:BTreeMap<_,_>=snapshot.widgets.iter().map(|n|(n.address,n)).collect();
    if by_address.len()!=snapshot.widgets.len() {return Err("Duplicate live addresses".into())}
    let rects:BTreeMap<_,_>=preview.rects.iter().map(|r|(r.id.as_str(),r)).collect();
    struct Walker<'a>{by_address:&'a BTreeMap<u32,&'a LiveWidget>,rects:&'a BTreeMap<&'a str,&'a crate::layout::Rect>,rows:Vec<Row>,seen:BTreeSet<u32>,viewport:[i32;2]}
    impl Walker<'_> {
        fn extra(&mut self,live:&LiveWidget,path:String) {
            let mut queue=vec![(live,path)];
            while let Some((l,path))=queue.pop() {
                if !self.seen.insert(l.address){continue}
                self.rows.push(Row{path:path.clone(),id:format!("live:{:08x}",l.address),status:"extra_live".into(),raw_delta:None,absolute_delta:None,preview_raw:None,live_raw:Some(l.relative),preview_absolute:None,live_absolute:Some(l.absolute),diagnostics:vec!["Runtime/generated widget has no source counterpart; excluded from source geometry match count".into()]});
                for child in l.children.iter().filter_map(|a|self.by_address.get(a).copied()){queue.push((child,format!("{path}/{}",child.path_id)));}
            }
        }
        fn visit(&mut self,node:&Widget,live:Option<&LiveWidget>,path:String,ancestors_visible:bool,ambiguous:bool) {
            let r=self.rects.get(node.id.as_str());
            let visible=live.is_some_and(|l|l.visible==1&&l.flags&0x400000==0)&&ancestors_visible;
            let live_on_screen=live.is_some_and(|l|l.absolute[2]>0&&l.absolute[3]>0&&l.absolute[0]<self.viewport[0]&&l.absolute[1]<self.viewport[1]&&l.absolute[2]>l.absolute[0]&&l.absolute[3]>l.absolute[1]);
            let raw_delta=live.zip(r).map(|(l,r)|delta(r.raw_relative,l.relative));
            let absolute_delta=if visible&&live_on_screen {live.zip(r).map(|(l,r)|delta([r.x,r.y,r.x+r.w,r.y+r.h],l.absolute))}else{None};
            let status=if ambiguous {"ambiguous"} else if live.is_none(){"missing_live"}else if !visible {"hidden_live"}else if r.is_none(){"missing_preview"}else if !live_on_screen{"offscreen_live"}else if raw_delta==Some([0;4])&&absolute_delta==Some([0;4]){"exact"}else{"mismatch"};
            let mut diagnostics=r.map(|r|r.diagnostics.clone()).unwrap_or_default();
            if !visible||!live_on_screen {diagnostics.push("Stored absolute rectangle may be stale; absolute comparison omitted".into());}
            self.rows.push(Row{path:path.clone(),id:node.id.clone(),status:status.into(),raw_delta,absolute_delta,preview_raw:r.map(|r|r.raw_relative),live_raw:live.map(|l|l.relative),preview_absolute:r.map(|r|[r.x,r.y,r.x+r.w,r.y+r.h]),live_absolute:live.map(|l|l.absolute),diagnostics});
            if let Some(l)=live {if !self.seen.insert(l.address){return}}
            for child in &node.children {
                let matches:Vec<_>=live.into_iter().flat_map(|l|&l.children).filter_map(|a|self.by_address.get(a).copied()).filter(|l|l.path_id==child.name).collect();
                self.visit(child,if matches.len()==1{Some(matches[0])}else{None},format!("{path}/{}",child.name),visible,matches.len()>1);
            }
            if let Some(l)=live {for child in l.children.iter().filter_map(|a|self.by_address.get(a).copied()) {
                if !node.children.iter().any(|n|n.name==child.path_id) {
                    self.extra(child,format!("{path}/{}",child.path_id));
                }
            }}
        }
    }
    let mut walker=Walker{by_address:&by_address,rects:&rects,rows:vec![],seen:BTreeSet::new(),viewport:[width,height]};
    walker.visit(tree,Some(roots[0]),tree.name.clone(),true,false);
    let mut counts=BTreeMap::new();for r in &walker.rows {*counts.entry(r.status.clone()).or_default()+=1;}
    Ok(Report{captured_utc:snapshot.captured_utc,binary_sha256:snapshot.binary_sha256,viewport:[width,height],counts,rows:walker.rows,diagnostics:vec!["Non-atomic live snapshot; repeat stable captures before accepting parity. Visibility/clip and runtime controller inputs are separate from source XML. An exact rectangle does not prove pixels, property/state correctness, or complete dialog fidelity.".into()]})
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn retained_island_updates_match_visible_geometry_without_native_extents_as_inputs(){
        let repo=crate::editor::discover_repo().unwrap();
        // External nonbinding barrier comes from source geometry in a separate scenario.
        let mut other=crate::editor::Editor::new(&repo).unwrap();
        let bar=other.open("menudefinitions/infobar.dlg").unwrap();
        let bar_tree=bar.tree.unwrap();
        let bar_capture=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/infobar-live-1440x1080.json")).unwrap();
        let bar_visibility=scenario(&bar_capture,&bar_tree,1440,1080).unwrap().visibility;
        let bar_geometry=other.preview_layout_scenario(bar.revision,1440,1080,false,&BTreeMap::new(),&bar_visibility).unwrap();
        fn named<'a>(node:&'a Widget,name:&str)->Option<&'a Widget>{if node.name==name{Some(node)}else{node.children.iter().find_map(|c|named(c,name))}}
        let wares=named(&bar_tree,"WARES_PANEL").unwrap();
        let rect=bar_geometry.rects.iter().find(|r|r.id==wares.id).unwrap();
        let edge=rect.x+rect.w;assert_eq!(edge,336);
        let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/island-panel-geometry-live.json")).unwrap();
        let mut editor=crate::editor::Editor::new(&repo).unwrap();
        let view=editor.open("menudefinitions/infobar_island_info.dlg").unwrap();let tree=view.tree.unwrap();
        let visibility=scenario(&json,&tree,1440,1080).unwrap().visibility;
        let mut runtime=crate::captions::Runtime{current_city_name:Some("First Island".into()),layout:crate::layout::Scenario{passes:1,island_info:Some(crate::layout::IslandInfoScenario{parent_viewport:[1440,1080],wares_right:edge})},..Default::default()};
        let first=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&visibility,&BTreeMap::new(),"eng",&runtime).unwrap();
        assert_eq!(first.passes[0].root_raw,[476,1,817,229]);
        assert!(!first.passes[0].identical_to_previous);
        runtime.layout.passes=32;
        let settled=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&visibility,&BTreeMap::new(),"eng",&runtime).unwrap();
        let report=compare(&json,&tree,&settled,1440,1080).unwrap();
        assert_eq!(report.counts.get("exact"),Some(&12));assert_eq!(report.counts.get("hidden_live"),Some(&45));assert!(!report.counts.contains_key("mismatch"));
        assert_eq!(settled.passes.last().unwrap().root_raw,[604,1,836,229]);assert!(settled.passes.last().unwrap().identical_to_previous);
        // History differs when the external barrier binds; never hide it by auto-centering.
        runtime.layout.passes=1;runtime.layout.island_info.as_mut().unwrap().wares_right=650;
        let clamped=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&visibility,&BTreeMap::new(),"eng",&runtime).unwrap();
        assert_eq!(clamped.passes[0].root_raw,[650,1,991,229]);
        runtime.layout.island_info.as_mut().unwrap().parent_viewport=[1280,1024];
        assert!(editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&visibility,&BTreeMap::new(),"eng",&runtime).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn original_mixed_case_fit_retains_fertility_list_height() {
        let repo=crate::editor::discover_repo().unwrap();
        let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/island-panel-geometry-live.json")).unwrap();
        let mut editor=crate::editor::Editor::new(&repo).unwrap();
        let view=editor.open("menudefinitions/infobar_island_info.dlg").unwrap();
        let tree=view.tree.unwrap();
        let scenario=scenario(&json,&tree,1440,1080).unwrap();
        let runtime=crate::captions::Runtime{current_city_name:Some("First Island".into()),..Default::default()};
        let preview=editor.preview_layout_text_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&scenario.visibility,&BTreeMap::new(),"eng",&runtime).unwrap();
        let report=compare(&json,&tree,&preview,1440,1080).unwrap();
        let list=report.rows.iter().find(|r|r.path.ends_with("/FERTILITIES_LIST")).unwrap();
        assert_eq!(list.preview_raw,Some([2,0,86,41]));
        assert_eq!(list.raw_delta,Some([0;4]));
        // The untranslated controller/lifecycle remains a real whole-panel mismatch.
        assert_eq!(list.status,"mismatch");
        assert!(list.absolute_delta.is_some_and(|d|d!=[0;4]));
    }
    use super::*;
    #[test]
    fn mismatches_hidden_and_viewport_are_not_silently_normalized() {
        let root=Widget{id:"0".into(),kind:"LABEL".into(),name:"TEST".into(),properties:vec![],layers:vec![],warnings:vec![],children:vec![],handlers:vec![]};
        let preview=crate::layout::preview(&root,1,100,100,false,&BTreeMap::new(),&BTreeMap::new());
        let json=r#"{"schema":2,"captured_utc":"fixture","binary_sha256":"fixture","gui_screen":{"GuiSize":[100,100],"Scale":[1,1]},"widgets":[{"Address":1,"PathId":"TEST","Relative":[0,0,10,10],"Absolute":[0,0,10,10],"Visible":1,"Flags":0,"Children":[]}]}"#;
        let report=compare(json,&root,&preview,100,100).unwrap();
        assert_eq!(report.rows[0].status,"mismatch");
        assert_eq!(report.rows[0].raw_delta,Some([0,0,-10,-10]));
        assert!(compare(json,&root,&preview,101,100).is_err());
        let hidden=json.replace("\"Visible\":1","\"Visible\":0");
        let report=compare(&hidden,&root,&preview,100,100).unwrap();
        assert_eq!(report.rows[0].status,"hidden_live");assert!(report.rows[0].absolute_delta.is_none());
        assert!(compare(&json.replace("[1,1]","[0.5,0.5]"),&root,&preview,100,100).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn live_infobar_fixture_uses_visibility_but_never_rectangles_as_inputs() {
        let repo=crate::editor::discover_repo().unwrap();
        let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/infobar-live-1440x1080.json")).unwrap();
        let manifest:serde_json::Value=serde_json::from_str(&json).unwrap();
        let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/infobar.dlg").unwrap();
        use sha2::Digest;
        assert_eq!(format!("{:x}",sha2::Sha256::digest(view.original.as_bytes())),manifest["source_sha256"].as_str().unwrap());
        assert_eq!(view.xml,view.original,"Use the original infobar fixture, not an edited working copy");
        let tree=view.tree.unwrap();let scenario=scenario(&json,&tree,1440,1080).unwrap();
        let preview=editor.preview_layout_scenario(view.revision,1440,1080,false,&BTreeMap::new(),&scenario.visibility).unwrap();
        let report=compare(&json,&tree,&preview,1440,1080).unwrap();
        assert_eq!(report.counts.get("exact"),Some(&25));
        assert!(!report.counts.contains_key("mismatch"));
        assert_eq!(report.counts.get("hidden_live"),Some(&51));
        assert_eq!(report.counts.get("offscreen_live"),Some(&1));
        assert_eq!(report.counts.get("extra_live"),Some(&2));
    }
}
