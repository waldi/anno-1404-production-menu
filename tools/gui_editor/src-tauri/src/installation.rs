use anno_gui_editor::editor::Editor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, ops::{Deref, DerefMut}, path::{Path, PathBuf}, process::Command, sync::{Mutex, MutexGuard, atomic::{AtomicBool, Ordering}}, time::Duration};

pub struct Store(pub Mutex<Option<Editor>>);
pub struct Ready<'a>(MutexGuard<'a, Option<Editor>>);
impl Store {
    pub fn lock(&self) -> Result<Ready<'_>, String> {
        let guard=self.0.lock().map_err(|e|e.to_string())?;
        if guard.is_none(){return Err("Select a game installation first".into())}
        Ok(Ready(guard))
    }
}
impl Deref for Ready<'_>{type Target=Editor;fn deref(&self)->&Editor{self.0.as_ref().unwrap()}}
impl DerefMut for Ready<'_>{fn deref_mut(&mut self)->&mut Editor{self.0.as_mut().unwrap()}}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Selection { pub game: String, pub explorer: String, pub prepared: String, pub fingerprint: String }
#[derive(Clone, Default, Serialize)]
pub struct Status { pub selection: Selection, pub ready: bool, pub preparing: bool, pub message: String }
pub struct Installation { pub root: PathBuf, pub status: Mutex<Status>, pub cancel: AtomicBool }

fn validate(game:&str, explorer:&str)->Result<(PathBuf,PathBuf),String>{
    let game=provider_path(PathBuf::from(game).canonicalize().map_err(|_|"Game directory does not exist")?);
    let explorer=provider_path(PathBuf::from(explorer).canonicalize().map_err(|_|"RDA Explorer directory does not exist")?);
    if !["Addon.exe", "Anno1404Addon.exe"].iter().any(|name| game.join(*name).is_file()) {
        return Err("Select an Anno 1404 game root containing Addon.exe or Anno1404Addon.exe".into());
    }
    for file in ["addon/data1.rda", "maindata/data1.rda"] {
        if !game.join(file).is_file(){return Err(format!("Selected game root is missing {file}"))}
    }
    if !explorer.join("RDAExplorer.dll").is_file(){return Err("Select the unpacked RDA Explorer folder containing RDAExplorer.dll".into())}
    Ok((game,explorer))
}
// PowerShell Join-Path rejects Windows verbatim paths returned by canonicalize.
fn provider_path(path:PathBuf)->PathBuf {
    #[cfg(windows)] {
        let text=path.to_string_lossy();
        if let Some(unc)=text.strip_prefix("\\\\?\\UNC\\"){return PathBuf::from(format!("\\\\{unc}"))}
        if let Some(local)=text.strip_prefix("\\\\?\\"){return PathBuf::from(local)}
    }
    path
}
fn fingerprint(game:&Path)->Result<String,String>{
    let mut records=vec![];
    for group in ["maindata","addon"]{
        for entry in fs::read_dir(game.join(group)).map_err(|e|e.to_string())?{
            let entry=entry.map_err(|e|e.to_string())?;
            if entry.path().extension().is_some_and(|e|e.eq_ignore_ascii_case("rda")){
                let m=entry.metadata().map_err(|e|e.to_string())?;
                records.push(format!("{group}/{:?}:{}:{:?}",entry.file_name(),m.len(),m.modified().map_err(|e|e.to_string())?));
            }
        }
    }
    records.sort();Ok(format!("{:x}",Sha256::digest(format!("prep-v1:{}:{}",game.display(),records.join("\n")))))
}
fn command()->Command{
    let mut c=Command::new("powershell.exe");
    #[cfg(windows)] { use std::os::windows::process::CommandExt; c.creation_flags(0x08000000); }
    c.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass"]);c
}
fn native_machine(bytes:&[u8])->Result<u16,String>{
    if bytes.get(..2)!=Some(b"MZ"){return Err("RDA Explorer zlib.dll is not a Windows DLL".into())}
    let offset=bytes.get(0x3c..0x40).ok_or("Truncated zlib.dll header")?;
    let offset=u32::from_le_bytes(offset.try_into().unwrap()) as usize;
    if bytes.get(offset..offset+4)!=Some(b"PE\0\0"){return Err("Invalid zlib.dll PE header".into())}
    let machine=bytes.get(offset+4..offset+6).ok_or("Truncated zlib.dll PE header")?;
    Ok(u16::from_le_bytes(machine.try_into().unwrap()))
}
fn extraction_command(explorer:&Path)->Result<Command,String>{
    let bytes=fs::read(explorer.join("zlib.dll")).map_err(|e|format!("The RDA Explorer folder must include its zlib.dll: {e}"))?;
    let host=match native_machine(&bytes)?{0x14c=>"SysWOW64",0x8664=>"System32",other=>return Err(format!("Unsupported zlib.dll architecture: {other:#x}"))};
    let windows=std::env::var_os("SystemRoot").ok_or("SystemRoot is unavailable")?;
    let executable=PathBuf::from(windows).join(host).join("WindowsPowerShell/v1.0/powershell.exe");
    if !executable.is_file(){return Err(format!("Required PowerShell host missing: {}",executable.display()))}
    let mut c=Command::new(executable);
    #[cfg(windows)] {use std::os::windows::process::CommandExt;c.creation_flags(0x08000000);}
    c.args(["-NoProfile","-NonInteractive","-ExecutionPolicy","Bypass"]);Ok(c)
}
pub fn restore(root:PathBuf)->(Installation,Option<Editor>){
    let selection=fs::read(root.join("selection.json")).ok().and_then(|b|serde_json::from_slice::<Selection>(&b).ok()).unwrap_or_default();
    let mut status=Status{selection:selection.clone(),..Status::default()};
    let editor=(||{
        let (game,_)=validate(&selection.game,&selection.explorer)?;
        if fingerprint(&game)?!=selection.fingerprint{return Err("Game archives changed. Prepare the installation again.".to_string())}
        let prepared=PathBuf::from(&selection.prepared).canonicalize().map_err(|e|e.to_string())?;
        let cache=root.join("installations").canonicalize().map_err(|e|e.to_string())?;
        if !prepared.starts_with(&cache){return Err("Invalid prepared asset location".into())}
        Editor::with_workspace(&prepared,&workspace(&root,&game))
    })();
    let editor=match editor{Ok(e)=>{status.ready=true;Some(e)},Err(e)=>{if !selection.game.is_empty(){status.message=e}None}};
    (Installation{root,status:Mutex::new(status),cancel:AtomicBool::new(false)},editor)
}
fn workspace(root:&Path,game:&Path)->PathBuf{root.join("workspaces").join(format!("{:x}",Sha256::digest(game.to_string_lossy().as_bytes())))}

#[tauri::command]
pub fn installation_status(state:tauri::State<Installation>)->Result<Status,String>{Ok(state.status.lock().map_err(|e|e.to_string())?.clone())}
#[tauri::command]
pub fn cancel_preparation(state:tauri::State<Installation>){state.cancel.store(true,Ordering::SeqCst);}
#[tauri::command]
pub async fn choose_directory()->Result<Option<String>,String>{
    tauri::async_runtime::spawn_blocking(||{
        let output=command().args(["-STA","-Command","Add-Type -AssemblyName System.Windows.Forms; $picker = New-Object System.Windows.Forms.FolderBrowserDialog; $picker.ShowNewFolderButton = $false; if ($picker.ShowDialog() -eq 'OK') { [Console]::OutputEncoding = [Text.Encoding]::UTF8; [Console]::Write($picker.SelectedPath) }"]).output().map_err(|e|e.to_string())?;
        if !output.status.success(){return Err(String::from_utf8_lossy(&output.stderr).into_owned())}
        let path=String::from_utf8_lossy(&output.stdout).trim().to_string();Ok(if path.is_empty(){None}else{Some(path)})
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn choose_definition_file()->Result<Option<String>,String>{
    tauri::async_runtime::spawn_blocking(||{
        let script="Add-Type -AssemblyName System.Windows.Forms; $picker = New-Object System.Windows.Forms.OpenFileDialog; $picker.Filter = 'Anno GUI definitions (*.dlg;*.tpl)|*.dlg;*.tpl|Dialog (*.dlg)|*.dlg|Template (*.tpl)|*.tpl'; $picker.CheckFileExists = $true; if ($picker.ShowDialog() -eq 'OK') { [Console]::OutputEncoding = [Text.Encoding]::UTF8; [Console]::Write($picker.FileName) }";
        let output=command().args(["-STA","-Command",script]).output().map_err(|e|e.to_string())?;
        if !output.status.success(){return Err(String::from_utf8_lossy(&output.stderr).into_owned())}
        let path=String::from_utf8_lossy(&output.stdout).trim().to_string();Ok(if path.is_empty(){None}else{Some(path)})
    }).await.map_err(|e|e.to_string())?
}
#[tauri::command]
pub async fn prepare_installation(game:String,explorer:String, app:tauri::AppHandle)->Result<Status,String>{
    use tauri::Manager;
    let state=app.state::<Installation>();
    {let mut s=state.status.lock().map_err(|e|e.to_string())?;if s.preparing{return Err("Preparation already running".into())}s.preparing=true;s.message="Validating folders…".into();}
    state.cancel.store(false,Ordering::SeqCst);
    tauri::async_runtime::spawn_blocking(move ||{
        let state=app.state::<Installation>();
        let result=(||->Result<Selection,String>{
            let (game,explorer)=validate(&game,&explorer)?;
            let fp=fingerprint(&game)?;
            let cache=state.root.join("installations");fs::create_dir_all(&cache).map_err(|e|e.to_string())?;
            let prepared=cache.join(format!("{}-{}",fp,std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            fs::create_dir(&prepared).map_err(|e|e.to_string())?;
            let outcome=(||->Result<Selection,String>{
                let script=prepared.join("prepare.ps1");fs::write(&script,include_str!("../../prepare-installation.ps1")).map_err(|e|e.to_string())?;
                let log=fs::File::create(prepared.join("preparation.log")).map_err(|e|e.to_string())?;
                let mut child=extraction_command(&explorer)?.arg("-File").arg(provider_path(script)).arg("-Game").arg(&game).arg("-Explorer").arg(&explorer).arg("-Output").arg(provider_path(prepared.clone())).stdout(log.try_clone().map_err(|e|e.to_string())?).stderr(log).spawn().map_err(|e|e.to_string())?;
                loop {
                    if state.cancel.load(Ordering::SeqCst){let _=child.kill();let _=child.wait();return Err("Preparation cancelled".into())}
                    if let Some(exit)=child.try_wait().map_err(|e|e.to_string())?{
                        if !exit.success(){return Err(format!("Asset preparation failed: {}",fs::read_to_string(prepared.join("preparation.log")).unwrap_or_default().chars().rev().take(3000).collect::<String>().chars().rev().collect::<String>()))}break
                    }
                    if let Ok(message)=fs::read_to_string(prepared.join("progress.txt")){state.status.lock().map_err(|e|e.to_string())?.message=message;}
                    std::thread::sleep(Duration::from_millis(200));
                }
                if fingerprint(&game)?!=fp{return Err("Game archives changed during preparation; retry".into())}
                let editor=Editor::with_workspace(&prepared,&workspace(&state.root,&game))?;
                if editor.catalog().files.is_empty(){return Err("No GUI definitions found".into())}
                if state.cancel.load(Ordering::SeqCst){return Err("Preparation cancelled".into())}
                let selection=Selection{game:game.to_string_lossy().into_owned(),explorer:explorer.to_string_lossy().into_owned(),prepared:prepared.to_string_lossy().into_owned(),fingerprint:fp};
                // Persist only after preparation and catalog construction succeed.
                let editor_state=app.state::<crate::State>();
                let mut active=editor_state.0.0.lock().map_err(|e|e.to_string())?;
                anno_gui_editor::editor::atomic_write(&state.root.join("selection.json"),&serde_json::to_vec_pretty(&selection).map_err(|e|e.to_string())?)?;
                *active=Some(editor);
                Ok(selection)
            })();
            if outcome.is_err(){let canonical=prepared.canonicalize().map_err(|e|e.to_string())?;if canonical.starts_with(cache.canonicalize().map_err(|e|e.to_string())?){let _=fs::remove_dir_all(canonical);}}
            outcome
        })();
        let mut s=state.status.lock().map_err(|e|e.to_string())?;s.preparing=false;
        match result{Ok(selection)=>{s.selection=selection;s.ready=true;s.message="Ready".into();Ok(s.clone())},Err(e)=>{s.message=e.clone();Err(e)}}
    }).await.map_err(|e|e.to_string())?
}

#[tauri::command]
pub fn setup_smoke_folders()->Result<(String,String),String>{
    if !std::env::args().any(|a|a=="--setup-smoke-test"){return Err("Not a setup smoke-test run".into())}
    let repo=anno_gui_editor::editor::discover_repo()?;
    Ok((repo.join("Anno 1404 Gold Edition").to_string_lossy().into_owned(),repo.join("rda-explorer").to_string_lossy().into_owned()))
}
#[tauri::command]
pub fn setup_smoke_complete(success:bool,message:String,app:tauri::AppHandle,state:tauri::State<Installation>)->Result<(),String>{
    if !std::env::args().any(|a|a=="--setup-smoke-test"){return Err("Not a setup smoke-test run".into())}
    fs::write(state.root.join("setup-smoke.json"),serde_json::to_vec_pretty(&serde_json::json!({"success":success,"message":message})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    app.exit(if success{0}else{1});Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reader_native_architecture_is_validated(){
        let mut pe=vec![0;70];pe[..2].copy_from_slice(b"MZ");pe[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());pe[64..68].copy_from_slice(b"PE\0\0");
        for arch in [0x14cu16,0x8664]{pe[68..70].copy_from_slice(&arch.to_le_bytes());assert_eq!(native_machine(&pe).unwrap(),arch);}
        assert!(native_machine(&pe[..69]).is_err());assert!(native_machine(b"not a DLL").is_err());
        let explorer=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../rda-explorer");
        if explorer.join("zlib.dll").is_file(){assert!(extraction_command(&explorer).unwrap().get_program().to_string_lossy().contains("SysWOW64"));}
    }
    fn fixture()->PathBuf {
        let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/setup-tests").join(format!("{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&root).unwrap();root
    }
    #[test]
    fn selection_accepts_gold_or_history_executable_and_detects_archive_changes(){
        let root=fixture();let game=root.join("game");let reader=root.join("reader");
        fs::create_dir_all(game.join("addon")).unwrap();fs::create_dir_all(game.join("maindata")).unwrap();fs::create_dir_all(&reader).unwrap();
        assert!(validate(game.to_str().unwrap(),reader.to_str().unwrap()).is_err());
        for file in ["addon/data1.rda","maindata/data1.rda"]{fs::write(game.join(file),b"fixture").unwrap();}
        fs::write(reader.join("RDAExplorer.dll"),b"fixture").unwrap();
        assert!(validate(game.to_str().unwrap(),reader.to_str().unwrap()).is_err());
        fs::write(game.join("Addon.exe"),b"gold fixture").unwrap();
        assert!(validate(game.to_str().unwrap(),reader.to_str().unwrap()).is_ok());
        fs::remove_file(game.join("Addon.exe")).unwrap();
        fs::write(game.join("Anno1404Addon.exe"),b"history fixture").unwrap();
        assert!(validate(game.to_str().unwrap(),reader.to_str().unwrap()).is_ok());
        let before=fingerprint(&game).unwrap();fs::write(game.join("addon/data1.rda"),b"changed archive").unwrap();assert_ne!(before,fingerprint(&game).unwrap());
    }
    #[test]
    fn saved_edits_survive_prepared_asset_generation_change(){
        let root=fixture();let workspace=root.join("workspace");
        for generation in ["first","second"]{
            let assets=root.join(generation);let gui=assets.join("data/addon/data1.rda/addondata/config/gui");fs::create_dir_all(&gui).unwrap();
            fs::write(gui.join("sample.dlg"),"<FRAME><NAME>Original</NAME></FRAME>").unwrap();
            let mut editor=Editor::with_workspace(&assets,&workspace).unwrap();
            if generation=="first"{
                fs::write(workspace.join("sample.dlg"),"<FRAME><NAME>Edited</NAME></FRAME>").unwrap();
            }
            let view=editor.open("sample.dlg").unwrap();assert!(view.xml.contains("Edited"));
        }
    }
    #[test]
    fn unconfigured_startup_is_valid_and_commands_report_setup_needed(){
        let root=fixture();let (state,editor)=restore(root);assert!(editor.is_none());assert!(!state.status.lock().unwrap().ready);
        let store=Store(Mutex::new(editor));assert!(store.lock().is_err());
    }
    #[test]
    #[cfg(windows)]
    fn canonical_paths_are_compatible_with_powershell_provider(){
        assert_eq!(provider_path(PathBuf::from(r"\\?\C:\Game")),PathBuf::from(r"C:\Game"));
        assert_eq!(provider_path(PathBuf::from(r"\\?\UNC\server\share")),PathBuf::from(r"\\server\share"));
    }
    #[test]
    #[ignore = "Run after prepare-installation.ps1 produces .tmp/setup-integration"]
    fn prepared_game_assets_load_without_repository_data(){
        let assets=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.tmp/setup-integration");
        let mut editor=Editor::with_workspace(&assets,&fixture().join("workspace")).unwrap();
        let catalog=editor.catalog();assert_eq!(catalog.files.len(),1873);assert!(catalog.errors.is_empty(),"{:?}",catalog.errors);
        let view=editor.open("menudefinitions/infobar.dlg").unwrap();assert_eq!(view.tree.unwrap().name,"INFOBAR");
        let key="backgrounds/final/minimap_frames/force_north_button.bkg";
        assert!(editor.asset(key).is_err());
        let thumbnail=editor.gallery_background(key,Some("startup-eng")).unwrap();
        assert!(!thumbnail.textures.is_empty());assert!(!thumbnail.state.is_empty());
        let localization=anno_gui_editor::localisation::inspect(&assets,"eng",100001,"Tooltip").unwrap();assert!(!localization.fonts.is_empty());assert!(!localization.texts.is_empty());
        let artwork=anno_gui_editor::background::texture_path_candidate(&assets,"data/graphics/ui/textures/ingame_gui_map_03.png",Some("startup-eng")).unwrap();assert!(artwork.starts_with(assets.canonicalize().unwrap()));
        let explicit=anno_gui_editor::background::texture_path_candidate(&assets,"data/graphics/ui/textures/ingame_gui_map_03.png",Some("maindata/patch1.rda")).unwrap();assert!(explicit.is_file());
        let icon=anno_gui_editor::icons::crop_candidate(&assets,122044,0,Some("startup-eng")).unwrap();assert!(!icon.png.is_empty());
    }
}
