//! Strict plain text placement gate. Cached dimensions/offsets are outputs only.
use serde::{Deserialize,Serialize};
use crate::native_text::{FontSpec,Screen};
#[derive(Deserialize)] #[serde(rename_all="PascalCase")]
struct Block {widget:u32,processed_id:u32,mode:u32,guid:i32,original_text:Option<String>,font_name:String,layout_flags:u32,layout_width:i32,layout_text_color:Option<u32>,client_size:Option<[i32;2]>,processed_size:Option<[i32;2]>}
#[derive(Deserialize)] #[serde(rename_all="PascalCase")]
struct Run {widget:u32,processed_id:u32,text:String,size:[i32;2],offset:[i32;2],line_rect:[i32;4],#[serde(default)] font:u32,#[serde(default)] color:u32,#[serde(default)] icon_guid:i32,#[serde(default)] icon_variation:i32,#[serde(default)] icon_clip:u8}
#[derive(Deserialize)] struct Capture {schema:u32,gui_screen:Screen,fonts:Vec<FontSpec>,blocks:Vec<Block>,runs:Vec<Run>,parts:Option<Vec<Run>>}
#[derive(Debug,Serialize,PartialEq)] pub struct Part {pub text:String,pub size:[i32;2],pub offset:[i32;2],pub line_rect:[i32;4]}
#[derive(Serialize)] pub struct Row {pub widget:u32,pub status:String,pub reason:String,pub predicted:Vec<Part>,pub rich_predicted:Vec<crate::text_layout::Part>,pub predicted_size:Option<[i32;2]>,pub captured_size:Option<[i32;2]>,pub captured_processed_size:Option<[i32;2]>,pub text_sources:Vec<String>}
#[derive(Serialize)] pub struct Report {pub exact:usize,pub mismatch:usize,pub unsupported:usize,pub rows:Vec<Row>,pub scope:String}
pub fn plain_line(text:&str,font:&FontSpec,scale:[f32;2],flags:u32,width:i32)->Result<Vec<Part>,String>{
    if text.is_empty()||text.chars().any(|c|c=='['||c==']'||c=='\\'||c.is_control()||c as u32>0xffff)||text.encode_utf16().count()>2047{return Err("Rich text, escapes, empty/control text and long tokens require a separate layout path".into())}
    if flags&!(6|0x40|0x80|0x100|0x800)!=0{return Err("Unimplemented layout flags".into())}
    let mut parts:Vec<Part>=vec![];let mut line:Vec<Part>=vec![];let mut x=0i32;let mut y=0;let mut height=0;
    let finish=|line:&mut Vec<Part>,parts:&mut Vec<Part>,x:i32,y:i32,height:i32|{
        let end=line.last().map(|p|p.offset[0]+p.size[0]).unwrap_or(0);
        let gap=if flags&0x40!=0{(width-end).max(0)}else{0};let shift=if flags&0x100!=0{gap/2}else if flags&0x800!=0{gap}else{0};
        for part in line.iter_mut(){part.offset[0]+=shift;part.line_rect=[0,y,x,y+height];}parts.append(line);
    };
    for token in text.split_inclusive(' '){let size=crate::native_text::measure(font,token,scale)?.size;
        if flags&0x40!=0&&x.checked_add(size[0]).ok_or("Text width overflow")?>width{
            if flags&0x80==0||x==0||size[0]>width{return Err("Oversized-word splitting or non-wrapping overflow required".into())}
            // DB0140 -> DC3480 -> DC4A70 -> DC4940 removes one trailing space
            // from the previous part and remeasures it, retaining pre-trim line bounds.
            if let Some(last)=line.last_mut(){if last.text.ends_with(' '){last.text.pop();last.size=crate::native_text::measure(font,&last.text,scale)?.size;}}
            finish(&mut line,&mut parts,x,y,height);y+=height;x=0;height=0;
        }
        line.push(Part{text:token.into(),size,offset:[x,y],line_rect:[0;4]});x=x.checked_add(size[0]).ok_or("Text width overflow")?;height=height.max(size[1]);
    }
    finish(&mut line,&mut parts,x,y,height);Ok(parts)
}
pub fn compare(json:&str)->Result<Report,String>{
    compare_with_language(json,None)
}
pub fn compare_with_language(json:&str,language:Option<(&std::path::Path,&str)>)->Result<Report,String>{
    if language.is_some_and(|(_,lang)|!matches!(lang,"eng"|"ger")){return Err("Choose extracted eng or ger resources".into())}
    if json.len()>32_000_000{return Err("Capture exceeds 32MB".into())}let capture:Capture=serde_json::from_str(json).map_err(|e|e.to_string())?;if capture.schema!=1{return Err("Expected text capture schema1".into())}
    let font_resources:Vec<crate::text_layout::Font>=if capture.parts.is_some(){let value:serde_json::Value=serde_json::from_str(json).map_err(|e|e.to_string())?;serde_json::from_value(value["fonts"].clone()).map_err(|e|e.to_string())?}else{vec![]};
    let mut report=Report{exact:0,mismatch:0,unsupported:0,rows:vec![],scope:if capture.parts.is_some(){"Captured all-part snapshots compare processed text bounds plus local text/icon dimensions, offsets, line bounds, font names, colors, GUIDs, variations and clip flags. Supported tags use recovered layout branches. Original strings, explicit language GUID sources, font settings, layout color and retained width are inputs only. Runtime substitutions, remaining tags, oversized words, owner origins and glyph/icon pixels remain separate gates."}else{"Legacy run-only snapshot checks plain token geometry. Original strings, explicit language GUID sources, FontName and retained LayoutWidth are independent inputs; inline icons/colors cannot be checked without an all-part capture. Owner origins and glyph pixels remain separate gates."}.into()};
    for block in capture.blocks {
        let mut text_sources=vec![];
        let mut rich_predicted=vec![];
        let mut predicted_size=None;
        let prediction=(||{
            let text=if block.mode==4{block.original_text.clone().ok_or("Original direct string missing")?}else if block.mode==1{
                let (repo,lang)=language.ok_or("Choose an extracted language explicitly for GUID text")?;
                let candidates=crate::localisation::inspect(repo,lang,block.guid,"")?.texts;
                let values:std::collections::BTreeSet<_>=candidates.iter().map(|c|c.value.clone()).collect();if values.len()!=1{return Err("GUID source missing or ambiguous".into())}
                text_sources=candidates.iter().map(|c|format!("{} SHA256 {}",c.source,c.sha256)).collect();values.into_iter().next().unwrap()
            }else{return Err("Pointer source text requires separate handling".into())};
            let fonts:Vec<_>=capture.fonts.iter().filter(|f|f.name==block.font_name).collect();if fonts.len()!=1{return Err("FontName must resolve uniquely".into())}
            if capture.parts.is_some(){let layout=crate::text_layout::layout_bounds_scenario(crate::text_layout::Input{text:&text,font:&block.font_name,fonts:&font_resources,scale:capture.gui_screen.scale,flags:block.layout_flags,width:block.layout_width,color:block.layout_text_color},None)?;predicted_size=Some(layout.size);rich_predicted=layout.parts;Ok(rich_predicted.iter().map(|p|Part{text:p.text.clone(),size:p.size,offset:p.offset,line_rect:p.line_rect}).collect())}
            else{plain_line(&text,fonts[0],capture.gui_screen.scale,block.layout_flags,block.layout_width)}
        })();
        let (status,reason,predicted)=match prediction {Err(reason)=>{report.unsupported+=1;("unsupported",reason,vec![])},Ok(parts)=>{let live:Vec<_>=capture.parts.as_ref().unwrap_or(&capture.runs).iter().filter(|r|r.widget==block.widget&&r.processed_id==block.processed_id).collect();let mut exact=live.len()==parts.len()&&live.iter().zip(&parts).all(|(r,p)|r.text==p.text&&r.size==p.size&&r.offset==p.offset&&r.line_rect==p.line_rect);
            if capture.parts.is_some(){exact &=block.client_size.is_none()||predicted_size==block.client_size;exact &=block.processed_size.is_none()||predicted_size==block.processed_size;exact &=live.len()==rich_predicted.len()&&live.iter().zip(&rich_predicted).all(|(r,p)|r.color==p.color&&match &p.font{Some(name)=>capture.fonts.iter().any(|f|f.address==r.font&&f.name==*name),None=>r.font==0&&Some(r.icon_guid)==p.icon_guid&&r.icon_variation==p.icon_variation&&(r.icon_clip!=0)==p.icon_clip});}
            if exact{report.exact+=1}else{report.mismatch+=1}(if exact{"exact"}else{"mismatch"},if exact{String::new()}else{format!("Predicted {} parts, captured {}. Compare processed bounds, token geometry and, for all-part captures, font/color/icon fields.",parts.len(),live.len())},parts)}};
        report.rows.push(Row{widget:block.widget,status:status.into(),reason,predicted,rich_predicted,predicted_size,captured_size:block.client_size,captured_processed_size:block.processed_size,text_sources});
    }Ok(report)
}
#[cfg(test)] mod tests {use super::*;
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn captured_inline_icons_breaks_fonts_and_colors_use_independent_inputs(){
        let repo=crate::editor::discover_repo().unwrap();
        for (name,exact) in [("text-inline-live-1440x1080.json",34),("text-inline-heldout-live-1440x1080.json",33)]{
            let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures").join(name)).unwrap();let report=compare_with_language(&json,Some((&repo,"eng"))).unwrap();assert_eq!((report.exact,report.mismatch,report.unsupported),(exact,0,39));
            let icon_widget=report.rows.iter().find(|r|r.status=="exact"&&r.rich_predicted.iter().any(|p|p.icon_guid.is_some())).unwrap().widget;
            let mut changed:serde_json::Value=serde_json::from_str(&json).unwrap();let block=changed["blocks"].as_array_mut().unwrap().iter_mut().find(|b|b["Widget"].as_u64()==Some(icon_widget as u64)).unwrap();block["ClientSize"][1]=serde_json::json!(9999);assert_eq!(compare_with_language(&changed.to_string(),Some((&repo,"eng"))).unwrap().mismatch,1);
            let mut changed:serde_json::Value=serde_json::from_str(&json).unwrap();let block=changed["blocks"].as_array_mut().unwrap().iter_mut().find(|b|b["Widget"].as_u64()==Some(icon_widget as u64)).unwrap();block["ProcessedSize"]=serde_json::json!([9999,9999]);assert_eq!(compare_with_language(&changed.to_string(),Some((&repo,"eng"))).unwrap().mismatch,1);
            for field in ["IconGuid","Color","Offset"] {let mut changed:serde_json::Value=serde_json::from_str(&json).unwrap();let part=changed["parts"].as_array_mut().unwrap().iter_mut().find(|p|p["Widget"].as_u64()==Some(icon_widget as u64)&&p["Font"]==0).unwrap();if field=="Offset"{part[field][0]=serde_json::json!(9999)}else{part[field]=serde_json::json!(9999)}assert_eq!(compare_with_language(&changed.to_string(),Some((&repo,"eng"))).unwrap().mismatch,1);}
        }
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"] fn explicit_english_guid_sources_extend_the_independent_gate(){let repo=crate::editor::discover_repo().unwrap();for (fixture,unsupported) in [("text-flow-live-1440x1080.json",51),("text-flow-heldout-live-1440x1080.json",52)]{let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures").join(fixture)).unwrap();let report=compare_with_language(&json,Some((&repo,"eng"))).unwrap();assert_eq!((report.exact,report.mismatch,report.unsupported),(22,0,unsupported));assert_eq!(report.rows.iter().filter(|r|r.status=="exact"&&!r.text_sources.is_empty()).count(),9);assert!(compare_with_language(&json,Some((&repo,"invalid"))).is_err());}}
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"] fn captured_direct_string_placement_uses_original_layout_width(){let repo=crate::editor::discover_repo().unwrap();let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/text-flow-live-1440x1080.json")).unwrap();let report=compare(&json).unwrap();assert!(report.exact>=10,"{}",serde_json::to_string(&report).unwrap());assert_eq!(report.mismatch,0);assert!(report.unsupported>0);
        let mut modified:serde_json::Value=serde_json::from_str(&json).unwrap();let widget=report.rows.iter().find(|r|r.status=="exact").unwrap().widget;let run=modified["runs"].as_array_mut().unwrap().iter_mut().find(|r|r["Widget"].as_u64()==Some(widget as u64)).unwrap();run["Offset"][0]=serde_json::json!(9999);assert_eq!(compare(&modified.to_string()).unwrap().mismatch,1);
    }
}
