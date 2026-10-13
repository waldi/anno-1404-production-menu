//! Explicit addon GUID/variation crop inspection; no inferred variation fallback.
use crate::background::{decode_dds, texture_path_candidate};
use base64::{engine::general_purpose::STANDARD, Engine};
use roxmltree::{Document, Node};
use serde::Serialize;
use std::{fs, path::Path};
type Result<T> = std::result::Result<T, String>;
fn child<'a, 'b>(n: Node<'a, 'b>, tag: &str) -> Option<Node<'a, 'b>> {
    n.children().find(|n| n.has_tag_name(tag))
}
fn text(n: Node, tag: &str) -> String {
    child(n, tag)
        .and_then(|n| n.text())
        .unwrap_or("")
        .trim()
        .into()
}
fn value(n: Node, tag: &str) -> Result<i32> {
    text(n, tag).parse().map_err(|_| format!("Invalid {tag}"))
}
#[derive(Serialize)]
pub struct Icon {
    pub guid: i32,
    pub variation: i32,
    pub file_id: i32,
    pub index: i32,
    pub source: [u32; 4],
    pub texture: String,
    pub texture_source:String,
    pub png: String,
    pub diagnostics: Vec<String>,
}
#[derive(Serialize)]
pub struct Sprite {
    pub id:String, pub destination:[i32;4], pub clip:[i32;4], pub icon:Icon,
}
#[derive(Serialize)]
pub struct IconPreview {pub sprites:Vec<Sprite>,pub diagnostics:Vec<String>}
pub fn placement(client:[i32;4],cell:[i32;2],space:[i32;4],stretch:bool)->[i32;4] {
    let x=client[0]+space[0];let y=client[1]+space[1];
    if stretch {[x,y,client[2]-space[2]-x,client[3]-space[3]-y]}else{[x,y,cell[0],cell[1]]}
}
pub fn preview(repo:&Path,tree:&crate::editor::Widget,layout:&crate::layout::Preview,viewport:[i32;2])->IconPreview {
    preview_candidate(repo,tree,layout,viewport,None)
}
pub fn preview_candidate(repo:&Path,tree:&crate::editor::Widget,layout:&crate::layout::Preview,viewport:[i32;2],archive:Option<&str>)->IconPreview {
    fn lookup<'a>(n:&'a crate::editor::Widget,id:&str)->Option<&'a crate::editor::Widget>{if n.id==id{Some(n)}else{n.children.iter().find_map(|c|lookup(c,id))}}
    let mut sprites=vec![];let mut diagnostics=vec!["Icon canvas uses explicit normal variation0 and unit effective scale. Label placement/defaults verified static; runtime variants, tint/disabled effects and pixel parity remain unverified.".into()];
    for rect in &layout.rects {
        let Some(node)=lookup(tree,&rect.id)else{continue};
        let property=|key:&str|node.properties.iter().find(|p|p.key==key).map(|p|p.value.as_str());
        let Some(guid)=property("ICONGUID")else{continue};let Ok(guid)=guid.parse::<i32>()else{diagnostics.push(format!("{}: ICONGUID needs runtime resolution",node.name));continue};if guid<0{continue}
        if node.kind!="LABEL" {diagnostics.push(format!("{}: {} icon renderer not implemented",node.name,node.kind));continue}
        let integer=|key:&str,default:i32|property(key).map(|v|if v.is_empty(){0}else{v.parse().unwrap_or(default)}).unwrap_or(default);
        match crop_candidate(repo,guid,0,archive) {
            Ok(icon)=>{let destination=placement(rect.client,[icon.source[2]as i32,icon.source[3]as i32],[integer("ICONSPACE/LEFT",0),integer("ICONSPACE/TOP",0),integer("ICONSPACE/WIDTH",0),integer("ICONSPACE/HEIGHT",0)],integer("ICONSTRETCH",0)!=0);
                let clip=if integer("ICONCLIP",1)==0{[0,0,viewport[0],viewport[1]]}else{rect.clip};
                if destination[2]>0&&destination[3]>0 {sprites.push(Sprite{id:node.id.clone(),destination,clip,icon})}
            },Err(e)=>diagnostics.push(format!("{}: {e}",node.name))
        }
    }
    IconPreview{sprites,diagnostics}
}
pub fn crop(repo: &Path, guid: i32, variation: i32) -> Result<Icon> {
    crop_candidate(repo,guid,variation,None)
}
pub fn crop_candidate(repo: &Path, guid: i32, variation: i32,archive:Option<&str>) -> Result<Icon> {
    let xml = fs::read_to_string(repo.join("data/addon/data1.rda/addondata/config/game/icons.xml"))
        .map_err(|e| e.to_string())?;
    let doc = Document::parse(&xml).map_err(|e| e.to_string())?;
    let records: Vec<_> = doc
        .root_element()
        .children()
        .filter(|n| n.is_element() && value(*n, "GUID") == Ok(guid))
        .collect();
    if records.len() != 1 {
        return Err("Missing/ambiguous addon GUID".into());
    }
    let variants: Vec<_> = child(records[0], "Icons")
        .ok_or("No icon variants")?
        .children()
        .filter(|n| n.is_element() && value(*n, "VariationID").unwrap_or(0) == variation)
        .collect();
    if variants.len() != 1 {
        return Err("Missing/ambiguous explicit variation; no fallback selected".into());
    }
    let file_id = value(variants[0], "IconFileID")?;
    let index = value(variants[0], "IconIndex")?;
    let map =
        fs::read_to_string(repo.join("data/maindata/data1.rda/data/config/gui/iconfilemap.xml"))
            .map_err(|e| e.to_string())?;
    let doc = Document::parse(&map).map_err(|e| e.to_string())?;
    let files: Vec<_> = doc
        .root_element()
        .children()
        .filter(|n| n.is_element() && value(*n, "IconFileID") == Ok(file_id))
        .collect();
    if files.len() != 1 {
        return Err("Missing/ambiguous icon file map".into());
    }
    let texture = text(files[0], "IconFilename");
    let (cw, ch) = (
        value(files[0], "IconWidth")?,
        value(files[0], "IconHeight")?,
    );
    if cw <= 0 || ch <= 0 || index < 0 {
        return Err("Invalid icon geometry".into());
    }
    let path=texture_path_candidate(repo, &texture,archive)?;
    let texture_source=path.strip_prefix(repo).unwrap_or(&path).to_string_lossy().replace('\\',"/");
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let (w, h, pixels) = decode_dds(&bytes)?;
    let (cw, ch) = (cw as u32, ch as u32);
    let columns = w / cw;
    if columns == 0 {
        return Err("Cell exceeds texture".into());
    }
    let (x, y) = ((index as u32 % columns) * cw, (index as u32 / columns) * ch);
    if x + cw > w || y + ch > h {
        return Err("Icon index exceeds atlas".into());
    }
    let mut cropped = Vec::with_capacity((cw * ch * 4) as usize);
    for row in y..y + ch {
        let start = ((row * w + x) * 4) as usize;
        cropped.extend_from_slice(&pixels[start..start + (cw * 4) as usize]);
    }
    let mut png = vec![];
    {
        let mut encoder = png::Encoder::new(&mut png, cw, ch);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .map_err(|e| e.to_string())?
            .write_image_data(&cropped)
            .map_err(|e| e.to_string())?;
    }
    Ok(Icon{guid,variation,file_id,index,source:[x,y,cw,ch],texture,texture_source,png:format!("data:image/png;base64,{}",STANDARD.encode(png)),diagnostics:vec!["Explicit available artwork candidate; row-major atlas indexing confirmed static. Effective live file origin, runtime variation selection, effects and pixel parity remain unverified.".into()]})
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unit_scale_iconspace_far_fields_are_insets_and_unstretched_icons_keep_cell_size(){
        assert_eq!(placement([10,20,110,80],[46,46],[3,4,7,8],false),[13,24,46,46]);
        assert_eq!(placement([10,20,110,80],[46,46],[3,4,7,8],true),[13,24,90,48]);
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn coin_and_explicit_variation() {
        let repo = crate::editor::discover_repo().unwrap();
        let icon = crop(&repo, 1160, 0).unwrap();
        assert_eq!(icon.file_id, 116);
        assert_eq!(icon.index, 242);
        assert_eq!(icon.source, [0, 506, 46, 46]);
        assert!(icon.png.starts_with("data:image/png;base64,"));
        assert!(crop(&repo, 1160, 999).is_err());
    }
}
