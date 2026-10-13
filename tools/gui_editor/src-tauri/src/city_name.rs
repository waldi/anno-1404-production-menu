//! CURRENT_CITY_NAME selection from independent scenario inputs (0075C7B0).
//! The values are not cached GUI text and never trigger game calls.
use serde::{Deserialize,Serialize};
#[derive(Deserialize)]#[serde(rename_all="PascalCase")]
pub struct Input{pub player:i32,pub names:Vec<String>,pub primary_scores:Vec<i32>,pub secondary_counts:Vec<i32>}
#[derive(Serialize)]pub struct Resolved{pub name:String,pub branch:String,pub player:Option<usize>}
#[derive(Serialize)]pub struct Imported{pub runtime:crate::captions::Runtime,pub selection:Option<Resolved>,pub sources:Vec<String>,pub scope:String}
pub fn import(repo:&std::path::Path,language:&str,json:&str)->Result<Imported,String>{
    if json.len()>1_000_000{return Err("City scenario JSON exceeds1MB".into())}
    if !matches!(language,"eng"|"ger"){return Err("Choose extracted eng or ger language".into())}
    let value:serde_json::Value=serde_json::from_str(json).map_err(|e|e.to_string())?;
    if value.get("schema").is_none(){let runtime:crate::captions::Runtime=serde_json::from_value(value).map_err(|e|e.to_string())?;runtime.validate()?;return Ok(Imported{runtime,selection:None,sources:vec![],scope:"Explicit display-name and text-refresh scenario. No automatic game-state binding or refresh-history verification.".into()})}
    if value["schema"]!=3{return Err("Use schema3 full city-name inputs; older cursor-mirror captures are insufficient".into())}
    #[derive(Deserialize)]struct Capture{source:Input}
    let capture:Capture=serde_json::from_value(value).map_err(|e|e.to_string())?;
    let mut selected=resolve(&capture.source,"")?;let mut sources=vec![];
    if selected.player.is_none(){let candidates=crate::localisation::inspect(repo,language,140038,"")?.texts;let values:std::collections::BTreeSet<_>=candidates.iter().map(|c|c.value.clone()).collect();if values.len()!=1{return Err("FallbackGUID140038 source missing or ambiguous".into())}selected.name=values.into_iter().next().unwrap();sources=candidates.iter().map(|c|format!("{} SHA256 {}",c.source,c.sha256)).collect();}
    Ok(Imported{runtime:crate::captions::Runtime{current_city_name:Some(selected.name.clone()),..Default::default()},selection:Some(selected),sources,scope:"Selection from independent schema3 island inputs and original localization fallback. Snapshot is non-atomic; full callback/live pixel parity pending.".into()})
}
pub fn resolve(input:&Input,fallback:&str)->Result<Resolved,String>{
    if input.names.len()!=30||input.primary_scores.len()!=8||input.secondary_counts.len()!=30{return Err("City-name scenario requires30 names,8 primary scores and30 secondary counts".into())}
    if input.names.iter().any(|v|v.encode_utf16().count()>512||v.chars().any(|c|c as u32>0xffff||c.is_control())){return Err("City-name scenario contains unsupported string units".into())}
    if let Ok(player)=usize::try_from(input.player){if let Some(name)=input.names.get(player).filter(|v|!v.is_empty()){return Ok(Resolved{name:name.clone(),branch:"local-player".into(),player:Some(player)})}}
    for (branch,scores) in [("primary-score",&input.primary_scores),("secondary-count",&input.secondary_counts)]{
        let mut best=0;let mut selected=None;
        for (player,score) in scores.iter().enumerate(){if !input.names[player].is_empty()&&*score>best{best=*score;selected=Some(player)}}
        if let Some(player)=selected{return Ok(Resolved{name:input.names[player].clone(),branch:branch.into(),player:Some(player)})}
    }
    Ok(Resolved{name:fallback.into(),branch:"localisation-guid140038".into(),player:None})
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn manual_and_captured_scenario_contracts_are_separate(){
        let repo=std::path::Path::new(".");assert_eq!(import(repo,"eng",r#"{"current_city_name":"Explicit city"}"#).unwrap().runtime.current_city_name.as_deref(),Some("Explicit city"));
        assert!(import(repo,"eng",r#"{"schema":1,"tag_values":{"CURRENT_CITY_NAME":"Cursor city"}}"#).err().unwrap().contains("schema3"));
        assert!(import(repo,"eng",r#"{"unknown_name":"x"}"#).is_err());assert!(import(repo,"fra",r#"{}"#).is_err());
    }
    #[test]fn fallback_priority_positive_scores_ties_and_invalid_player(){
        let mut input=Input{player:29,names:vec![String::new();30],primary_scores:vec![0;8],secondary_counts:vec![0;30]};
        input.names[0]="First".into();input.names[1]="Tie".into();input.names[8]="Secondary".into();input.primary_scores[0]=7;input.primary_scores[1]=7;input.secondary_counts[8]=100;
        assert_eq!(resolve(&input,"Island").unwrap().player,Some(0));
        input.names[29]="Local".into();assert_eq!(resolve(&input,"Island").unwrap().branch,"local-player");
        input.player=-1;assert_eq!(resolve(&input,"Island").unwrap().name,"First");
        input.primary_scores.fill(-1);assert_eq!(resolve(&input,"Island").unwrap().player,Some(8));
        input.secondary_counts.fill(0);assert_eq!(resolve(&input,"Island").unwrap().name,"Island");
        input.names.pop();assert!(resolve(&input,"Island").is_err());
    }
}
