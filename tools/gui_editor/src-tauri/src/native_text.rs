//! Own-process GDI metrics using recovered FontContainer parameters.
//! Matching metrics do not establish D3DX glyph rasterization parity.
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(rename_all="PascalCase")]
pub struct FontSpec {pub address:u32,pub name:String,pub face:String,pub flags:u32,pub height:i32,pub weight:i32}
#[derive(Deserialize)]
#[serde(rename_all="PascalCase")]
pub struct Run {pub widget:u32,pub part:u32,pub text:String,pub size:[i32;2],pub font:u32}
#[derive(Deserialize)]
#[serde(rename_all="PascalCase")]
pub struct Screen {pub scale:[f32;2]}
#[derive(Deserialize)]
pub struct Snapshot {pub schema:u32,pub captured_utc:String,pub binary_sha256:String,pub gui_screen:Screen,pub fonts:Vec<FontSpec>,pub runs:Vec<Run>}
#[derive(Serialize)]
pub struct Metrics {pub size:[i32;2],pub physical_size:[i32;2],pub selected_face:String,pub height:i32,pub ascent:i32,pub descent:i32,pub internal_leading:i32,pub external_leading:i32,pub max_char_width:i32}
#[derive(Serialize)]
pub struct Row {pub widget:u32,pub part:u32,pub text:String,pub font:FontSpec,pub live_size:[i32;2],pub measured:Metrics,pub delta:[i32;2]}
#[derive(Serialize)]
pub struct Report {pub captured_utc:String,pub binary_sha256:String,pub counts:BTreeMap<String,usize>,pub rows:Vec<Row>,pub diagnostics:Vec<String>}
fn check_scale(scale:[f32;2])->Result<(),String>{if scale.iter().any(|s|!s.is_finite()||*s<=0.0||*s>8.0){Err("Invalid screen scale".into())}else{Ok(())}}
pub fn font_spec(xml:&str)->Result<FontSpec,String>{
    let doc=roxmltree::Document::parse(xml).map_err(|e|e.to_string())?;
    if !doc.root_element().has_tag_name("Font"){return Err("Expected a Font definition".into())}
    let mut spec=FontSpec{address:0,name:String::new(),face:String::new(),height:12,weight:0,flags:0};
    for node in doc.root_element().children().filter(|n|n.is_element()) {
        let value=node.text().unwrap_or("").trim();
        let integer=||if value.is_empty(){Ok(0)}else{value.parse::<i32>().map_err(|_|format!("Invalid font integer {}",node.tag_name().name()))};
        match node.tag_name().name() {
            "Name"=>spec.name=value.into(),
            "FontName"=>spec.face=if value.len()>=32{"Arial".into()}else{value.into()},
            "FontSize"=>spec.height=integer()?,
            "RelativeFontSize"=>{spec.height=integer()?;spec.flags|=1;},
            "Weight"=>{spec.weight=integer()?;spec.flags|=0x10;},
            "DefaultColor"=>spec.flags|=0x80,
            "Flag"=>spec.flags|=match value {"TextureFont"=>0x20,"Italic"=>2,"Underline"=>4,"StrikeOut"=>8,"Outline"=>0x100,"Shadow"=>0x40,"Antialiasing"=>0x200,_=>0},
            _=>{}
        }
    }
    if spec.face.is_empty(){return Err("Missing FontName; constructor fallback is not guessed".into())}Ok(spec)
}
#[cfg(windows)]
mod gdi {
    use super::*;
    use std::ptr::null_mut;
    use windows_sys::Win32::{Foundation::SIZE,Graphics::Gdi::*};
    pub struct Session {dc:HDC,font:HFONT,old:HGDIOBJ,spec:FontSpec,scale:[f32;2]}
    impl Session {
        pub fn new(spec:&FontSpec,scale:[f32;2])->Result<Self,String>{
            check_scale(scale)?;
            if !(-1024..=1024).contains(&spec.height)||spec.weight<0||spec.weight>1000||spec.face.is_empty()||spec.face.encode_utf16().count()>31||spec.face.contains('\0'){return Err("Invalid font parameters".into())}
            let mut lf:LOGFONTW=unsafe{std::mem::zeroed()};
            lf.lfHeight=if spec.flags&1!=0{(spec.height as f32*scale[1]+0.5)as i32}else{spec.height};
            lf.lfWeight=if spec.flags&0x10!=0{spec.weight}else{0};
            lf.lfItalic=((spec.flags>>1)&1)as u8;lf.lfUnderline=((spec.flags>>2)&1)as u8;lf.lfStrikeOut=((spec.flags>>3)&1)as u8;
            lf.lfOutPrecision=4;lf.lfQuality=if spec.flags&0x200!=0{5}else{3};
            for (i,c) in spec.face.encode_utf16().enumerate(){lf.lfFaceName[i]=c}
            unsafe {
                let dc=CreateCompatibleDC(null_mut());if dc.is_null(){return Err("CreateCompatibleDC failed".into())}
                let font=CreateFontIndirectW(&lf);if font.is_null(){DeleteDC(dc);return Err("CreateFontIndirectW failed".into())}
                let old=SelectObject(dc,font);if old.is_null()||old as isize == -1{DeleteObject(font);DeleteDC(dc);return Err("SelectObject font failed".into())}
                Ok(Self{dc,font,old,spec:spec.clone(),scale})
            }
        }
        pub fn measure(&self,text:&str)->Result<Metrics,String>{
            let units:Vec<u16>=text.encode_utf16().collect();if units.len()>16384{return Err("Text run exceeds 16K UTF16 units".into())}
            unsafe {
                let mut size:SIZE=std::mem::zeroed();let mut tm:TEXTMETRICW=std::mem::zeroed();
                if !units.is_empty()&&GetTextExtentPoint32W(self.dc,units.as_ptr(),units.len()as i32,&mut size)==0{return Err("GetTextExtentPoint32W failed".into())}
                if GetTextMetricsW(self.dc,&mut tm)==0{return Err("GetTextMetricsW failed".into())}
                let mut face=[0u16;128];let n=GetTextFaceW(self.dc,128,face.as_mut_ptr());if n<=0{return Err("GetTextFaceW failed".into())}
                let selected_face=String::from_utf16(&face[..face.iter().position(|c|*c==0).unwrap_or(n as usize)]).map_err(|e|e.to_string())?;
                if !units.is_empty()&&self.spec.flags&0x100!=0{size.cx+=units.len()as i32*2;size.cy+=2}
                Ok(Metrics{size:[(size.cx as f32/self.scale[0])as i32,(size.cy as f32/self.scale[1])as i32],physical_size:[size.cx,size.cy],selected_face,height:tm.tmHeight,ascent:tm.tmAscent,descent:tm.tmDescent,internal_leading:tm.tmInternalLeading,external_leading:tm.tmExternalLeading,max_char_width:tm.tmMaxCharWidth})
            }
        }
    }
    impl Drop for Session {fn drop(&mut self){unsafe{SelectObject(self.dc,self.old);DeleteObject(self.font);DeleteDC(self.dc);}}}
}
pub fn measure(spec:&FontSpec,text:&str,scale:[f32;2])->Result<Metrics,String>{
    #[cfg(windows)]{gdi::Session::new(spec,scale)?.measure(text)}
    #[cfg(not(windows))]{let _=(spec,text,scale);Err("Native GDI metrics require Windows".into())}
}
pub fn compare(json:&str)->Result<Report,String>{
    if json.len()>32_000_000{return Err("Text capture exceeds 32MB".into())}
    let capture:Snapshot=serde_json::from_str(json).map_err(|e|e.to_string())?;
    if capture.schema!=1{return Err("Use capture-text-reference.ps1 schema1".into())}check_scale(capture.gui_screen.scale)?;
    let fonts:BTreeMap<_,_>=capture.fonts.into_iter().map(|f|(f.address,f)).collect();
    let mut report=Report{captured_utc:capture.captured_utc,binary_sha256:capture.binary_sha256,counts:BTreeMap::new(),rows:vec![],diagnostics:vec!["Own-process GDI parameters reproduce the recovered measurement path. Captured font settings are explicit inputs; run dimensions are comparison outputs only. Cached run sizes/non-atomic snapshots may be stale. Glyph pixels, line breaking and full text layout remain separate gates.".into()]};
    for run in capture.runs {
        let font=fonts.get(&run.font).ok_or("Run font missing from snapshot")?;
        let measured=measure(font,&run.text,capture.gui_screen.scale)?;
        let delta=[measured.size[0]-run.size[0],measured.size[1]-run.size[1]];
        *report.counts.entry(if delta==[0,0]{"exact"}else{"mismatch"}.into()).or_default()+=1;
        if measured.selected_face!=font.face{report.diagnostics.push(format!("{} substituted with {} by GDI",font.face,measured.selected_face))}
        report.rows.push(Row{widget:run.widget,part:run.part,text:run.text,font:font.clone(),live_size:run.size,measured,delta});
    }
    Ok(report)
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]fn native_height_outline_and_scale(){
        let font=FontSpec{address:0,name:"fixture".into(),face:"Times New Roman".into(),flags:0x201,height:18,weight:0};
        let normal=measure(&font,"Anno 1404",[1.0,1.0]).unwrap();assert_eq!(normal.selected_face,"Times New Roman");assert!(normal.size[0]>0&&normal.size[1]>0);
        let outlined=measure(&FontSpec{flags:font.flags|0x100,..font.clone()},"Anno 1404",[1.0,1.0]).unwrap();assert_eq!(outlined.size,[normal.size[0]+18,normal.size[1]+2]);
        assert_eq!(measure(&font,"",[1.0,1.0]).unwrap().size,[0,0]);assert!(measure(&font,"x",[0.0,1.0]).is_err());
        assert!(measure(&FontSpec{height:i32::MIN,..font.clone()},"x",[1.0,1.0]).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn named_font_definition_matches_captured_default_parameters(){
        let repo=crate::editor::discover_repo().unwrap();
        let inspection=crate::localisation::inspect(&repo,"eng",-1,"Tooltip").unwrap();
        for candidate in inspection.fonts {
            let font=font_spec(&candidate.value).unwrap();assert_eq!(font.name,"Tooltip");assert_eq!(font.face,"Times New Roman");assert_eq!(font.height,18);assert_eq!(font.flags,673);
        }
        let ordered=font_spec("<Font><FontName>Arial</FontName><RelativeFontSize>20</RelativeFontSize><FontSize>12</FontSize></Font>").unwrap();assert_eq!(ordered.height,12);assert_eq!(ordered.flags&1,1);
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn captured_native_run_metrics_agree_without_using_captured_sizes_as_inputs(){
        let repo=crate::editor::discover_repo().unwrap();
        let capture=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/text-runs-live-1440x1080.json")).unwrap();
        let report=compare(&capture).unwrap();assert_eq!(report.counts.get("exact"),Some(&121));assert!(!report.counts.contains_key("mismatch"));
        let mut changed:serde_json::Value=serde_json::from_str(&capture).unwrap();changed["runs"][0]["Size"][0]=serde_json::json!(9999);
        let negative=compare(&changed.to_string()).unwrap();assert_eq!(negative.counts.get("mismatch"),Some(&1));
    }
}
