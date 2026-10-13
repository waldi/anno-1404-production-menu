//! Read-only corpus inventory. JSON on stdout; never saves editor working copies.
use roxmltree::Document;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

fn increment(map: &mut BTreeMap<String, usize>, key: impl Into<String>) {
    *map.entry(key.into()).or_default() += 1;
}
fn visit(
    root: &Path,
    dir: &Path,
    files: &mut BTreeMap<String, Value>,
    counts: &mut BTreeMap<String, BTreeMap<String, usize>>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            continue;
        }
        let path = entry.path();
        if path.is_dir() {
            visit(root, &path, files, counts)?;
            continue;
        }
        let extension = path.extension().and_then(|x| x.to_str()).unwrap_or("");
        if !["dlg", "tpl", "stp", "bkg"].contains(&extension) {
            continue;
        }
        let bytes = fs::read(&path)?;
        let xml = std::str::from_utf8(&bytes)?;
        let doc = Document::parse(xml)?;
        let key = path
            .strip_prefix(root)?
            .to_string_lossy()
            .replace('\\', "/");
        let mut features = BTreeMap::<String, usize>::new();
        for node in doc.descendants().filter(|n| n.is_element()) {
            let tag = node.tag_name().name();
            if [
                "FRAME",
                "LABEL",
                "BUTTON",
                "SLIDER",
                "EDITFIELD",
                "BOXLAYOUT",
                "CARDLAYOUT",
                "COMBOBOX",
                "LISTBOX",
                "PROGRESSBAR",
                "CIRCULARLYPROGRESSBAR",
                "RENDERTARGET",
                "VIDEOPANEL",
                "STRUCTTEMPLATEINSTANCE",
            ]
            .contains(&tag)
            {
                increment(counts.entry("widget_types".into()).or_default(), tag);
                increment(&mut features, format!("widget:{tag}"));
            }
            if node.children().any(|n| n.is_element()) {
                continue;
            }
            let value = node.text().unwrap_or("").trim();
            increment(counts.entry("leaf_tags".into()).or_default(), tag);
            let category = match tag {
                "LayoutName" => Some("background_states"),
                "FONT" => Some("font_names"),
                "flags"
                    if node
                        .parent()
                        .and_then(|p| p.parent())
                        .is_some_and(|p| p.has_tag_name("IconList")) =>
                {
                    Some("tile_flags")
                }
                "LINE_ORDER"
                | "LINECOLUMNCOUNT"
                | "BOXLAYOUT_BEHAVIOUR"
                | "LAYOUTMODEH"
                | "LAYOUTMODEV"
                | "CLIPPINGDISABLED" => Some("layout_values"),
                _ => None,
            };
            if let Some(category) = category {
                let feature = if category == "layout_values" {
                    format!("{tag}={value}")
                } else {
                    value.into()
                };
                increment(counts.entry(category.into()).or_default(), &feature);
                increment(&mut features, format!("{category}:{feature}"));
            }
        }
        files.insert(key, json!({"sha256":format!("{:x}", Sha256::digest(&bytes)),"bytes":bytes.len(),"features":features}));
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let repo = anno_gui_editor::editor::discover_repo()?;
    let args:Vec<_>=std::env::args().skip(1).collect();
    if args.first().is_some_and(|s|s=="--city-name-scenario"){
        if args.len()!=3{return Err("Usage: gui-audit --city-name-scenario scenario.json eng|ger".into())}
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::city_name::import(&repo,&args[2],&fs::read_to_string(&args[1])?)?)?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--glyph-raster") {
        if args.len()!=2{return Err("Usage: gui-audit --glyph-raster independent-input.json".into())}
        let input=serde_json::from_str::<anno_gui_editor::native_glyph::RasterInput>(&fs::read_to_string(&args[1])?)?;
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::native_glyph::render_run(&input)?)?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--captions") {
        if !(5..=6).contains(&args.len()){return Err("Usage: gui-audit --captions definition width height eng|ger [scenario.json]".into())}
        let mut editor=anno_gui_editor::editor::Editor::new(&repo)?;let view=editor.open(&args[1])?;
        let runtime=if let Some(file)=args.get(5){anno_gui_editor::city_name::import(&repo,&args[4],&fs::read_to_string(file)?)?.runtime}else{Default::default()};
        let layout=editor.preview_layout_text_scenario(view.revision,args[2].parse()?,args[3].parse()?,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),&args[4],&runtime)?;
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::captions::preview_runtime(&repo,&args[4],view.tree.as_ref().ok_or("No tree")?,&layout,&BTreeMap::new(),&runtime))?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--glyph-metadata") {
        if args.len()!=2{return Err("Usage: gui-audit --glyph-metadata glyph-capture.json".into())}
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::native_glyph::compare(&fs::read_to_string(&args[1])?)?)?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--text-flow") {
        if args.len()!=2&&args.len()!=4{return Err("Usage: gui-audit --text-flow text-capture.json [--language eng|ger]".into())}
        let language=if args.len()==4{if args[2]!="--language"{return Err("Expected --language".into())}Some((repo.as_path(),args[3].as_str()))}else{None};
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::text_flow::compare_with_language(&fs::read_to_string(&args[1])?,language)?)?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--button-states") {
        if args.len()!=2{return Err("Usage: gui-audit --button-states schema1-button-capture.json".into())}
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::button_state::compare(&fs::read_to_string(&args[1])?)?)?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--text-metrics") {
        if args.len()!=2{return Err("Usage: gui-audit --text-metrics schema1-text-capture.json".into())}
        println!("{}",serde_json::to_string_pretty(&anno_gui_editor::native_text::compare(&fs::read_to_string(&args[1])?)?)?);return Ok(())
    }
    if args==["--background-coverage"] {
        let editor=anno_gui_editor::editor::Editor::new(&repo)?;
        let root=repo.join("data/addon/data1.rda/addondata/config/gui");
        let mut textures:BTreeMap<String,Result<(u32,u32),String>>=BTreeMap::new();let mut formats=BTreeMap::new();let mut results=BTreeMap::new();let mut counts=BTreeMap::new();
        for key in editor.catalog().files.into_iter().filter(|f|f.ends_with(".bkg")) {
            let xml=fs::read_to_string(root.join(&key))?;let doc=Document::parse(&xml)?;
            for state in doc.descendants().filter(|n|n.has_tag_name("LayoutName")).map(|n|n.text().unwrap_or("").trim()) {
                let result=(||->Result<(),String>{
                    let bg=anno_gui_editor::background::grid(&xml,state,1000,500)?;
                    for tile in bg.tiles {
                        if tile.flags & 32 != 0 || tile.destination[2] <= 0 || tile.destination[3] <= 0 {continue}
                        if !textures.contains_key(&tile.texture) {
                            let decoded=(||->Result<(u32,u32),String>{let path=anno_gui_editor::background::texture_path(&repo,&tile.texture)?;let b=fs::read(path).map_err(|e|e.to_string())?;
                                let format=if b.len()>=128&&u32::from_le_bytes(b[80..84].try_into().unwrap())&4!=0{String::from_utf8_lossy(&b[84..88]).into_owned()}else{"uncompressed".into()};increment(&mut formats,format);
                                let(w,h,_)=anno_gui_editor::background::decode_dds(&b)?;Ok((w,h))})();
                            textures.insert(tile.texture.clone(),decoded);
                        }
                        let &(w,h)=textures[&tile.texture].as_ref().map_err(|e|e.clone())?;
                        if tile.source[0]+tile.source[2]>w as i32||tile.source[1]+tile.source[3]>h as i32{return Err("Tile source exceeds texture".into())}
                    }Ok(())
                })();
                increment(&mut counts,if result.is_ok(){"decoded"}else{"unsupported"});results.insert(format!("{key}:{state}"),result.err());
            }
        }
        println!("{}",serde_json::to_string_pretty(&json!({"scope":"Every explicit extracted background state at1000x500; parser,source bounds and original DDS decode only. No pixel parity implied.","counts":counts,"texture_formats":formats,"unique_textures":textures.len(),"results":results}))?);return Ok(())
    }
    if args.first().is_some_and(|s|s=="--geometry") {
        let mut geometry_args=args.clone();
        let runtime=if let Some(index)=geometry_args.iter().position(|a|a=="--text-scenario"){
            let file=geometry_args.get(index+1).ok_or("--text-scenario needs a JSON file")?.clone();
            geometry_args.drain(index..=index+1);
            anno_gui_editor::city_name::import(&repo,"eng",&fs::read_to_string(file)?)?.runtime
        }else{Default::default()};
        let args=&geometry_args;
        if !(4..=6).contains(&args.len())||args.get(5).is_some_and(|a|a!="--captured-visibility") {return Err("Usage: gui-audit --geometry definition width height [schema-2-snapshot.json [--captured-visibility]] [--text-scenario scenario.json] (English)".into())}
        let width=args[2].parse()?;let height=args[3].parse()?;
        let mut editor=anno_gui_editor::editor::Editor::new(&repo)?;
        let view=editor.open(&args[1])?;
        let visibility=if args.len()==6 {anno_gui_editor::reference::scenario(&fs::read_to_string(&args[4])?,view.tree.as_ref().ok_or("No tree")?,width,height)?.visibility}else{BTreeMap::new()};
        let preview=editor.preview_layout_text_scenario(view.revision,width,height,false,&BTreeMap::new(),&visibility,&BTreeMap::new(),"eng",&runtime)?;
        let result=if let Some(path)=args.get(4) {
            let snapshot=fs::read_to_string(path)?;
            json!({"scenario_visibility_overrides":visibility.len(),"layout_scenario":runtime.layout,"passes":preview.passes,"diagnostics":preview.diagnostics,"report":anno_gui_editor::reference::compare(&snapshot,view.tree.as_ref().ok_or("No tree")?,&preview,width,height)?})
        }else{json!({"definition":view.key,"source_sha256":format!("{:x}",Sha256::digest(view.original.as_bytes())),"working_copy_sha256":format!("{:x}",Sha256::digest(view.xml.as_bytes())),"tree":view.tree,"preview":preview})};
        println!("{}",serde_json::to_string_pretty(&result)?);return Ok(())
    }
    if !args.is_empty(){return Err("Unknown arguments; use no arguments for inventory or --geometry".into())}
    let root = repo.join("data/addon/data1.rda/addondata/config/gui");
    let mut files = BTreeMap::new();
    let mut counts = BTreeMap::new();
    visit(&root, &root, &mut files, &mut counts)?;
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"schema":1,"scope":"extracted addon GUI sources; raw occurrences, not effective inherited values","confidence":"confirmed corpus observations; no rendering semantics inferred","source_root":"data/addon/data1.rda/addondata/config/gui","file_count":files.len(),"counts":counts,"files":files})
        )?
    );
    Ok(())
}
