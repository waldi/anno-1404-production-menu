//! Raw extracted localization inspection. No font substitution, markup evaluation,
//! archive precedence or game text rendering is inferred.
use roxmltree::Document;
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::BTreeSet,fs,path::Path};
#[derive(Serialize)]
pub struct Candidate {pub source:String,pub sha256:String,pub value:String}
#[derive(Serialize)]
pub struct Inspection {pub texts:Vec<Candidate>,pub fonts:Vec<Candidate>,pub diagnostics:Vec<String>}
pub fn font_document(repo:&Path,language:&str,name:&str,source:&str)->Result<String,String>{
    let candidates=inspect(repo,language,-1,name)?.fonts;
    let candidate=candidates.iter().find(|c|c.source==source).ok_or("Choose a staged font source explicitly")?;
    let path=repo.join(&candidate.source).canonicalize().map_err(|e|e.to_string())?;
    if !path.starts_with(repo.canonicalize().map_err(|e|e.to_string())?){return Err("Font source escapes repo".into())}
    let bytes=fs::read(path).map_err(|e|e.to_string())?;
    if format!("{:x}",Sha256::digest(&bytes))!=candidate.sha256{return Err("Font source changed during inspection".into())}decode(&bytes)
}
pub fn decode(bytes:&[u8])->Result<String,String>{
    if bytes.starts_with(&[0xff,0xfe])||bytes.starts_with(&[0xfe,0xff]){
        if bytes.len()%2!=0{return Err("Odd UTF-16 byte length".into())}
        let le=bytes[0]==0xff;
        let units:Vec<_>=bytes[2..].chunks_exact(2).map(|b|if le{u16::from_le_bytes([b[0],b[1]])}else{u16::from_be_bytes([b[0],b[1]])}).collect();
        String::from_utf16(&units).map_err(|e|e.to_string())
    }else{String::from_utf8(bytes.strip_prefix(&[0xef,0xbb,0xbf]).unwrap_or(bytes).to_vec()).map_err(|e|e.to_string())}
}
pub fn inspect(repo:&Path,language:&str,guid:i32,font:&str)->Result<Inspection,String>{
    if !["eng","ger"].contains(&language){return Err("Choose extracted eng or ger resources".into())}
    let mut result=Inspection{texts:vec![],fonts:vec![],diagnostics:vec!["Raw candidates from extracted resources. No archive precedence, multiline/text tags, substitutions, font aliasing or rasterization inferred. Selecting a language here does not change the game.".into()]};
    let mut definitions=vec![];
    let mut roots=vec![];
    for group in ["maindata","addon"] {
        let root=repo.join(format!("data/{group}/{language}0.rda/data/loca/{language}"));
        if !root.is_dir(){continue}
        roots.push(root.clone());
        let file=root.join(if group=="addon"{"localisation_addon.xml"}else{"localisation.xml"});
        if file.is_file(){definitions.push(file)}
    }
    let mut tables=BTreeSet::new();
    for definition in definitions {
        let bytes=fs::read(&definition).map_err(|e|e.to_string())?;
        let xml=decode(&bytes)?;let document=Document::parse(&xml).map_err(|e|e.to_string())?;
        let text=|node:roxmltree::Node,tag:&str|node.children().find(|n|n.has_tag_name(tag)).and_then(|n|n.text()).unwrap_or("").trim().to_string();
        for node in document.root_element().children().filter(|n|n.is_element()) {
            if node.has_tag_name("Font")&&text(node,"Name")==font {
                result.fonts.push(Candidate{source:definition.strip_prefix(repo).unwrap_or(&definition).to_string_lossy().replace('\\',"/"),sha256:format!("{:x}",Sha256::digest(&bytes)),value:xml[node.range()].into()});
            }
            if node.has_tag_name("Text") {
                let min=text(node,"GUIDRangeMin").parse::<i32>().unwrap_or(i32::MAX);let max=text(node,"GUIDRangeMax").parse::<i32>().unwrap_or(i32::MIN);
                if guid<min||guid>max{continue}
                let name=text(node,"Filename").replace('\\',"/");
                if name.split('/').any(|part|part.is_empty()||part==".."||part=="."||part.contains(':')){return Err("Invalid localization table path".into())}
                for root in &roots {
                    let file=root.join(&name);if !file.is_file(){continue}
                    let canonical=file.canonicalize().map_err(|e|e.to_string())?;
                    if !canonical.starts_with(root.canonicalize().map_err(|e|e.to_string())?){return Err("Localization file escapes extracted root".into())}
                    tables.insert(canonical);
                }
            }
        }
    }
    for table in tables {
        let bytes=fs::read(&table).map_err(|e|e.to_string())?;if bytes.len()>32_000_000{return Err("Localization table exceeds 32MB".into())}
        let content=decode(&bytes)?;
        for line in content.lines() {
            let Some((key,value))=line.split_once('=')else{continue};
            if key.trim().parse::<i32>()!=Ok(guid){continue}
            result.texts.push(Candidate{source:table.strip_prefix(repo).unwrap_or(&table).to_string_lossy().replace('\\',"/"),sha256:format!("{:x}",Sha256::digest(&bytes)),value:value.into()});
        }
    }
    if result.texts.is_empty()&&guid>=0{result.diagnostics.push(format!("GUID {guid}: no single-line candidate in staged range tables"))}
    if result.fonts.is_empty()&&!font.is_empty(){result.diagnostics.push(format!("Font {font}: no definition in staged localization XML"))}
    Ok(result)
}
#[cfg(test)]mod tests {
    use super::*;
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn strict_bom_decode_and_raw_markup(){
        assert_eq!(decode(&[0xff,0xfe,0x23,0]).unwrap(),"#");
        assert_eq!(decode(&[0xfe,0xff,0,0x23]).unwrap(),"#");
        assert!(decode(&[0xff,0xfe,0]).is_err());assert!(decode(&[0xff,0xfe,0,0xd8]).is_err());
        let repo=crate::editor::discover_repo().unwrap();
        if repo.join("data/maindata/eng0.rda/data/loca/eng/localisation.xml").is_file(){let data=inspect(&repo,"eng",100001,"Tooltip").unwrap();assert!(!data.fonts.is_empty());assert!(data.texts.iter().any(|c|c.value.contains("[AUTOSIZE_ICONGUID 122044 0]")));}
        assert!(inspect(&repo,"../eng",100001,"").is_err());
    }
}
