#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use anno_gui_editor::editor::{discover_repo, CatalogInfo, Editor, View};
use std::sync::Mutex;
mod installation;
struct State(installation::Store);
#[tauri::command]
fn current_view(state:tauri::State<State>)->Result<View,String>{state.0.lock().map_err(|e|e.to_string())?.view()}
#[tauri::command]
fn preview_progress(revision:u64,width:i32,height:i32,show_hidden:bool,cards:std::collections::BTreeMap<String,usize>,visibility:std::collections::BTreeMap<String,bool>,buttons:Option<std::collections::BTreeMap<String,anno_gui_editor::button_state::Input>>,inputs:std::collections::BTreeMap<String,anno_gui_editor::progress::Input>,archive_candidate:Option<String>,language:Option<String>,runtime:Option<anno_gui_editor::captions::Runtime>,state:tauri::State<State>)->Result<anno_gui_editor::progress::Render,String>{
    state.0.lock().map_err(|e|e.to_string())?.preview_progress_text_scenario(revision,width,height,show_hidden,&cards,&visibility,&buttons.unwrap_or_default(),&inputs,archive_candidate.as_deref(),language.as_deref().unwrap_or("eng"),&runtime.unwrap_or_default())
}
#[tauri::command]
fn native_font_metrics(language:String,font:String,source:String,text:String,scale:[f32;2],state:tauri::State<State>)->Result<anno_gui_editor::native_text::Metrics,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let candidates=anno_gui_editor::localisation::inspect(&editor.repo,&language,-1,&font)?;
    let candidate=candidates.fonts.into_iter().find(|c|c.source==source).ok_or("Choose a staged font source explicitly")?;
    if text.contains(['\r','\n']){return Err("This metric probe measures a single run; line layout is separate".into())}
    anno_gui_editor::native_text::measure(&anno_gui_editor::native_text::font_spec(&candidate.value)?,&text,scale)
}
#[tauri::command]
fn text_metrics_compare(snapshot:String)->Result<anno_gui_editor::native_text::Report,String>{anno_gui_editor::native_text::compare(&snapshot)}
#[tauri::command]
fn native_font_atlas(language:String,font:String,source:String,state:tauri::State<State>)->Result<anno_gui_editor::native_glyph::Atlas,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let candidate=anno_gui_editor::localisation::inspect(&editor.repo,&language,-1,&font)?.fonts.into_iter().find(|c|c.source==source).ok_or("Choose a staged font source explicitly")?;
    let document=anno_gui_editor::localisation::font_document(&editor.repo,&language,&font,&candidate.source)?;
    anno_gui_editor::native_glyph::build(&anno_gui_editor::native_glyph::font_from_document(&document,&font,[1.0,1.0])?)
}
#[tauri::command]
fn native_font_raster(language:String,font:String,source:String,text:String,color:u32,state:tauri::State<State>)->Result<anno_gui_editor::native_glyph::Raster,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let candidate=anno_gui_editor::localisation::inspect(&editor.repo,&language,-1,&font)?.fonts.into_iter().find(|c|c.source==source).ok_or("Choose a staged font source explicitly")?;
    let document=anno_gui_editor::localisation::font_document(&editor.repo,&language,&font,&candidate.source)?;
    anno_gui_editor::native_glyph::render_run(&anno_gui_editor::native_glyph::RasterInput{font:anno_gui_editor::native_glyph::font_from_document(&document,&font,[1.0,1.0])?,text,color})
}
#[tauri::command]
fn glyph_metadata_compare(snapshot:String)->Result<anno_gui_editor::native_glyph::Report,String>{anno_gui_editor::native_glyph::compare(&snapshot)}
#[tauri::command]
fn smoke_glyph_reference(state:tauri::State<State>)->Result<String,String>{
    if !std::env::args().any(|a|a=="--smoke-test"){return Err("Not a smoke-test run".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    std::fs::read_to_string(editor.repo.join("tools/gui_editor/fixtures/glyph-metadata-live-1440x1080.json")).map_err(|e|e.to_string())
}
#[tauri::command]
fn text_flow_compare(snapshot:String,language:Option<String>,state:tauri::State<State>)->Result<anno_gui_editor::text_flow::Report,String>{let editor=state.0.lock().map_err(|e|e.to_string())?;anno_gui_editor::text_flow::compare_with_language(&snapshot,language.as_deref().map(|lang|(editor.repo.as_path(),lang)))}
#[tauri::command]
fn smoke_text_flow_reference(state:tauri::State<State>)->Result<String,String>{
    if !std::env::args().any(|a|a=="--smoke-test"){return Err("Not a smoke-test run".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    std::fs::read_to_string(editor.repo.join("tools/gui_editor/fixtures/text-flow-live-1440x1080.json")).map_err(|e|e.to_string())
}
#[tauri::command]
fn smoke_text_inline_reference(heldout:bool,state:tauri::State<State>)->Result<String,String>{
    if !std::env::args().any(|a|a=="--smoke-test"){return Err("Not a smoke-test run".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let name=if heldout{"text-inline-heldout-live-1440x1080.json"}else{"text-inline-live-1440x1080.json"};
    std::fs::read_to_string(editor.repo.join("tools/gui_editor/fixtures").join(name)).map_err(|e|e.to_string())
}
#[tauri::command]
fn button_states_compare(snapshot:String)->Result<anno_gui_editor::button_state::Report,String>{anno_gui_editor::button_state::compare(&snapshot)}
#[tauri::command]
fn localisation_inspect(language:String,guid:i32,font:String,state:tauri::State<State>)->Result<anno_gui_editor::localisation::Inspection,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    anno_gui_editor::localisation::inspect(&editor.repo,&language,guid,&font)
}
#[tauri::command]
fn preview_icons(revision:u64,width:i32,height:i32,show_hidden:bool,cards:std::collections::BTreeMap<String,usize>,visibility:std::collections::BTreeMap<String,bool>,buttons:Option<std::collections::BTreeMap<String,anno_gui_editor::button_state::Input>>,archive_candidate:Option<String>,language:Option<String>,runtime:Option<anno_gui_editor::captions::Runtime>,state:tauri::State<State>)->Result<anno_gui_editor::icons::IconPreview,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let layout=editor.preview_layout_text_scenario(revision,width,height,show_hidden,&cards,&visibility,&buttons.unwrap_or_default(),language.as_deref().unwrap_or("eng"),&runtime.unwrap_or_default())?;
    Ok(anno_gui_editor::icons::preview_candidate(&editor.repo,&editor.view()?.tree.ok_or("No tree")?,&layout,[width,height],archive_candidate.as_deref()))
}
#[tauri::command]
fn smoke_reference(state:tauri::State<State>)->Result<String,String>{
    if !std::env::args().any(|a|a=="--smoke-test"){return Err("Not a smoke-test run".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    std::fs::read_to_string(editor.repo.join("tools/gui_editor/fixtures/infobar-live-1440x1080.json")).map_err(|e|e.to_string())
}
#[tauri::command]
fn smoke_text_reference(state:tauri::State<State>)->Result<String,String>{
    if !std::env::args().any(|a|a=="--smoke-test"){return Err("Not a smoke-test run".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    std::fs::read_to_string(editor.repo.join("tools/gui_editor/fixtures/text-runs-live-1440x1080.json")).map_err(|e|e.to_string())
}
#[tauri::command]
fn smoke_button_reference(state:tauri::State<State>)->Result<String,String>{
    if !std::env::args().any(|a|a=="--smoke-test"){return Err("Not a smoke-test run".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    std::fs::read_to_string(editor.repo.join("tools/gui_editor/fixtures/button-states-live-1440x1080.json")).map_err(|e|e.to_string())
}
#[tauri::command]
fn geometry_compare(snapshot:String, revision:u64,width:i32,height:i32,show_hidden:bool,cards:std::collections::BTreeMap<String,usize>,visibility:Option<std::collections::BTreeMap<String,bool>>,buttons:Option<std::collections::BTreeMap<String,anno_gui_editor::button_state::Input>>,language:Option<String>,runtime:Option<anno_gui_editor::captions::Runtime>,state:tauri::State<State>)->Result<anno_gui_editor::reference::Report,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let preview=editor.preview_layout_text_scenario(revision,width,height,show_hidden,&cards,&visibility.unwrap_or_default(),&buttons.unwrap_or_default(),language.as_deref().unwrap_or("eng"),&runtime.unwrap_or_default())?;
    let tree=editor.view()?.tree.ok_or("No widget tree")?;
    anno_gui_editor::reference::compare(&snapshot,&tree,&preview,width,height)
}
#[tauri::command]
fn geometry_scenario(snapshot:String,revision:u64,width:i32,height:i32,state:tauri::State<State>)->Result<anno_gui_editor::reference::Scenario,String>{
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    editor.preview_layout(revision,width,height,false,&std::collections::BTreeMap::new())?;
    anno_gui_editor::reference::scenario(&snapshot,&editor.view()?.tree.ok_or("No widget tree")?,width,height)
}
#[tauri::command]
fn icon_asset(
    guid: i32,
    variation: i32,
    archive_candidate:Option<String>,
    state: tauri::State<State>,
) -> Result<anno_gui_editor::icons::Icon, String> {
    let repo = state.0.lock().map_err(|e| e.to_string())?.repo.clone();
    anno_gui_editor::icons::crop_candidate(&repo, guid, variation,archive_candidate.as_deref())
}
#[tauri::command]
fn preview_layout(
    revision: u64,
    width: i32,
    height: i32,
    show_hidden: bool,
    cards: std::collections::BTreeMap<String, usize>,
    visibility:Option<std::collections::BTreeMap<String,bool>>,
    buttons:Option<std::collections::BTreeMap<String,anno_gui_editor::button_state::Input>>,
    language:Option<String>,
    runtime:Option<anno_gui_editor::captions::Runtime>,
    state: tauri::State<State>,
) -> Result<anno_gui_editor::layout::Preview, String> {
    state.0.lock().map_err(|e| e.to_string())?.preview_layout_text_scenario(
        revision,
        width,
        height,
        show_hidden,
        &cards,
        &visibility.unwrap_or_default(),
        &buttons.unwrap_or_default(),
        language.as_deref().unwrap_or("eng"),
        &runtime.unwrap_or_default(),
    )
}
#[tauri::command]
fn city_name_scenario(json:String,language:String,state:tauri::State<State>)->Result<anno_gui_editor::city_name::Imported,String>{let editor=state.0.lock().map_err(|e|e.to_string())?;anno_gui_editor::city_name::import(&editor.repo,&language,&json)}
#[tauri::command]
fn preview_captions(revision:u64,width:i32,height:i32,show_hidden:bool,cards:std::collections::BTreeMap<String,usize>,visibility:std::collections::BTreeMap<String,bool>,buttons:std::collections::BTreeMap<String,anno_gui_editor::button_state::Input>,language:String,runtime:Option<anno_gui_editor::captions::Runtime>,state:tauri::State<State>)->Result<anno_gui_editor::captions::Preview,String>{
    if !matches!(language.as_str(),"eng"|"ger"){return Err("Choose an extracted language".into())}
    let editor=state.0.lock().map_err(|e|e.to_string())?;
    let runtime=runtime.unwrap_or_default();
    let layout=editor.preview_layout_text_scenario(revision,width,height,show_hidden,&cards,&visibility,&buttons,&language,&runtime)?;
    Ok(anno_gui_editor::captions::preview_runtime(&editor.repo,&language,&editor.view()?.tree.ok_or("No tree")?,&layout,&buttons,&runtime))
}
#[tauri::command]
fn catalog(state: tauri::State<State>) -> Result<CatalogInfo, String> {
    Ok(state.0.lock().map_err(|e| e.to_string())?.catalog())
}
#[tauri::command]
fn open_definition(key: String, state: tauri::State<State>) -> Result<View, String> {
    state.0.lock().map_err(|e| e.to_string())?.open(&key)
}
#[tauri::command]
fn import_definition(relative_path:String,state:tauri::State<State>)->Result<View,String>{
    state.0.lock().map_err(|e|e.to_string())?.import_definition(&relative_path)
}
#[tauri::command]
fn edit_properties(
    revision: u64,
    id: String,
    edits: Vec<(String, Option<String>)>,
    state: tauri::State<State>,
) -> Result<View, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .edit_many(revision, &id, &edits)
}
#[tauri::command]
fn history(revision: u64, redo: bool, state: tauri::State<State>) -> Result<View, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .history(revision, redo)
}
#[tauri::command]
fn save_copy(revision: u64, state: tauri::State<State>) -> Result<View, String> {
    state.0.lock().map_err(|e| e.to_string())?.save(revision)
}
#[tauri::command]
fn background_preview(key: String, state: tauri::State<State>) -> Result<String, String> {
    state.0.lock().map_err(|e| e.to_string())?.asset(&key)
}
#[tauri::command]
fn background_thumbnail(key:String,archive_candidate:Option<String>,state:tauri::State<State>)->Result<anno_gui_editor::background::Background,String>{
    state.0.lock().map_err(|e|e.to_string())?.gallery_background(&key,archive_candidate.as_deref())
}
#[tauri::command]
fn background_render(
    key: String,
    layout_state: String,
    width: i32,
    height: i32,
    archive_candidate:Option<String>,
    state: tauri::State<State>,
) -> Result<anno_gui_editor::background::Background, String> {
    if let Some(archive)=archive_candidate{
        let editor=state.0.lock().map_err(|e|e.to_string())?;
        return editor.background_candidate(&key,&layout_state,width,height,&archive);
    }
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .background(&key, &layout_state, width, height)
}
#[tauri::command]
fn smoke_background(png: String, kind:Option<String>, state: tauri::State<State>) -> Result<(), String> {
    use base64::Engine;
    if !std::env::args().any(|a| a == "--smoke-test") {
        return Err("Not a smoke-test run".into());
    }
    if png.len() > 16_000_000 {
        return Err("Smoke PNG too large".into());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(
            png.strip_prefix("data:image/png;base64,")
                .ok_or("Expected PNG")?,
        )
        .map_err(|e| e.to_string())?;
    if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err("Invalid PNG".into());
    }
    let repo = state.0.lock().map_err(|e| e.to_string())?.repo.clone();
    let filename=match kind.as_deref(){None=>"objectmenuinside-render.png",Some("force-north-checked")=>"force-north-checked-render.png",Some("force-north-patch0")=>"force-north-patch0-render.png",Some("force-north-patch1")=>"force-north-patch1-render.png",Some("progress")=>"progress-render.png",_=>return Err("Unknown smoke artifact".into())};
    std::fs::write(
        repo.join("tools/gui_editor/.tmp").join(filename),
        bytes,
    )
    .map_err(|e| e.to_string())
}
#[tauri::command]
fn smoke_complete(
    success: bool,
    message: String,
    app: tauri::AppHandle,
    state: tauri::State<State>,
) -> Result<(), String> {
    if !std::env::args().any(|a| a == "--smoke-test") {
        return Err("Not a smoke-test run".into());
    }
    let repo = state.0.lock().map_err(|e| e.to_string())?.repo.clone();
    std::fs::write(
        repo.join("tools/gui_editor/.tmp/smoke.json"),
        serde_json::to_vec_pretty(&serde_json::json!({"success":success,"message":message}))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    app.exit(if success { 0 } else { 1 });
    Ok(())
}
fn main() {
    let result = (|| -> Result<(), String> {
        let smoke = std::env::args().any(|a| a == "--smoke-test");
        let setup_smoke=std::env::args().any(|a|a=="--setup-smoke-test");
        let data_dir=std::env::args().skip_while(|a|a!="--data-dir").nth(1);
        if setup_smoke && data_dir.is_none(){return Err("Setup smoke test requires --data-dir".into())}
        let repo = if smoke { discover_repo()? } else if let Some(path)=data_dir { std::path::PathBuf::from(path) } else {
            std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable")?).join("AnnoGUIStudio")
        };
        std::fs::create_dir_all(&repo).map_err(|e|e.to_string())?;
        let (installation, restored) = installation::restore(repo.clone());
        let editor = if smoke { Some(Editor::new(&repo)?) } else { restored };
        let cache = repo.join("tools/gui_editor/.webview");
        let log = repo.join("tools/gui_editor/.tmp/startup.log");
        let smoke = std::env::args().any(|a| a == "--smoke-test");
        std::fs::create_dir_all(log.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(
            &log,
            format!(
                "Catalog loaded: {} definitions\n",
                editor.as_ref().map(|e|e.catalog().files.len()).unwrap_or(0)
            ),
        )
        .map_err(|e| e.to_string())?;
        tauri::Builder::default()
            .manage(State(installation::Store(Mutex::new(editor))))
            .manage(installation)
            .invoke_handler(tauri::generate_handler![
                installation::setup_smoke_folders,
                installation::setup_smoke_complete,
                installation::installation_status,
                installation::choose_directory,
                installation::choose_definition_file,
                installation::prepare_installation,
                installation::cancel_preparation,
                current_view,
                catalog,
                icon_asset,
                localisation_inspect,
                native_font_metrics,
                native_font_atlas,
                native_font_raster,
                preview_captions,
                city_name_scenario,
                glyph_metadata_compare,
                smoke_glyph_reference,
                preview_progress,
                text_metrics_compare,
                text_flow_compare,
                smoke_text_flow_reference,
                smoke_text_inline_reference,
                button_states_compare,
                preview_layout,
                preview_icons,
                smoke_reference,
                smoke_text_reference,
                smoke_button_reference,
                geometry_compare,
                geometry_scenario,
                open_definition,
                import_definition,
                edit_properties,
                history,
                save_copy,
                background_preview,
                background_thumbnail,
                background_render,
                smoke_background,
                smoke_complete
            ])
            .setup(move |app| {
                let log = log.clone();
                tauri::WebviewWindowBuilder::new(
                    app,
                    "main",
                    tauri::WebviewUrl::App("index.html".into()),
                )
                .title("Anno 1404 — GUI Studio")
                .inner_size(1440.0, 900.0)
                .min_inner_size(1000.0, 650.0)
                .visible(!smoke && !setup_smoke)
                .data_directory(if smoke {
                    cache.join("smoke")
                } else {
                    cache.clone()
                })
                .initialization_script(if setup_smoke {
                    "window.__ANNO_SETUP_SMOKE__ = true; window.__ANNO_SMOKE__ = false;"
                } else if smoke {
                    "window.__ANNO_SMOKE__ = true;"
                } else {
                    "window.__ANNO_SMOKE__ = false;"
                })
                .on_page_load(move |_, payload| {
                    use std::io::Write;
                    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).open(&log) {
                        let _ = writeln!(f, "Webview page event: {:?}", payload.event());
                    }
                })
                .build()?;
                Ok(())
            })
            .run(tauri::generate_context!())
            .map_err(|e| e.to_string())
    })();
    if let Err(error) = result {
        eprintln!("{error}");
    }
}
