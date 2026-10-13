use base64::{engine::general_purpose::STANDARD, Engine};
use roxmltree::{Document, Node};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

type Result<T> = std::result::Result<T, String>;
const TYPES: &[&str] = &[
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
];
fn widget(n: Node) -> bool {
    n.is_element() && TYPES.contains(&n.tag_name().name())
}
fn children<'a, 'input>(n: Node<'a, 'input>) -> impl Iterator<Item = Node<'a, 'input>> {
    n.children().filter(|x| x.is_element())
}
fn child<'a, 'input>(n: Node<'a, 'input>, tag: &str) -> Option<Node<'a, 'input>> {
    children(n).find(|x| x.tag_name().name() == tag)
}
fn text(n: Node, tag: &str) -> String {
    child(n, tag)
        .and_then(|x| x.text())
        .unwrap_or("")
        .trim()
        .to_string()
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn parse(xml: &str) -> Result<Document<'_>> {
    Document::parse(xml).map_err(|e| e.to_string())
}
fn import_key(relative: &str) -> Result<String> {
    if relative.len() > 512 || relative.trim().is_empty() {
        return Err("Choose a repo-relative .dlg or .tpl path".into());
    }
    let relative = relative.replace('\\', "/");
    let parts: Vec<_> = relative.split('/').collect();
    if relative.starts_with('/')
        || relative.contains(':')
        || parts.iter().any(|part| part.is_empty() || *part == "." || *part == "..")
    {
        return Err("Import path must stay inside the repo and cannot use . or ..".into());
    }
    if !matches!(Path::new(&relative).extension().and_then(|v| v.to_str()), Some("dlg" | "tpl")) {
        return Err("Only .dlg or .tpl XML definitions can be imported".into());
    }
    Ok(relative)
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
fn opening_end(s: &str) -> Result<usize> {
    let mut quote = None;
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if c == '>' {
            return Ok(i + 1);
        }
    }
    Err("Invalid opening tag".into())
}

#[derive(Clone, Serialize, Debug)]
pub struct Property {
    pub key: String,
    pub value: String,
    pub source: String,
    pub local: bool,
}
#[derive(Clone, Serialize, Debug)]
pub struct Handler { pub event:String, pub source:String, pub actions:Vec<Property> }
#[derive(Clone, Serialize, Debug)]
pub struct Widget {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub properties: Vec<Property>,
    pub layers: Vec<Vec<Property>>,
    pub warnings: Vec<String>,
    pub children: Vec<Widget>,
    pub handlers: Vec<Handler>,
}
#[derive(Serialize)]
pub struct CatalogInfo {
    pub files: Vec<String>,
    pub counts: BTreeMap<String, usize>,
    pub repo: String,
    pub errors: Vec<String>,
}
#[derive(Serialize, Debug)]
pub struct View {
    pub key: String,
    pub xml: String,
    pub original: String,
    pub revision: u64,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub tree: Option<Widget>,
    pub states: Vec<String>,
    pub preview: Option<String>,
    pub saved_to: Option<String>,
}
#[derive(Clone)]
struct Session {
    key: String,
    xml: String,
    original: String,
    disk: Option<String>,
    undo: Vec<String>,
    redo: Vec<String>,
    revision: u64,
    saved_to: Option<String>,
}
pub struct Editor {
    pub repo: PathBuf,
    import_root: PathBuf,
    workspace: PathBuf,
    root: PathBuf,
    docs: BTreeMap<String, String>,
    overrides: BTreeMap<String, String>,
    templates: BTreeMap<String, Vec<String>>,
    errors: Vec<String>,
    session: Option<Session>,
    serial: u64,
}

fn walk(
    dir: &Path,
    root: &Path,
    docs: &mut BTreeMap<String, String>,
    errors: &mut Vec<String>,
) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_symlink() {
            continue;
        }
        let p = entry.path();
        if p.is_dir() {
            walk(&p, root, docs, errors)?;
        } else if matches!(
            p.extension().and_then(|x| x.to_str()),
            Some("dlg" | "tpl" | "stp" | "bkg")
        ) {
            let key = p
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            match fs::read_to_string(&p)
                .map_err(|e| e.to_string())
                .and_then(|s| {
                    parse(&s)?;
                    Ok(s)
                }) {
                Ok(s) => {
                    docs.insert(key, s);
                }
                Err(e) => errors.push(format!("{}: {}", key, e)),
            }
        }
    }
    Ok(())
}

fn collect(n: Node, source: &str, prefix: &str, out: &mut Vec<Property>) {
    for c in children(n).filter(|x| !widget(*x)) {
        let key = format!("{}{}", prefix, c.tag_name().name());
        if children(c).next().is_some() {
            collect(c, source, &(key + "/"), out);
        } else {
            out.push(Property {
                key,
                value: c.text().unwrap_or("").trim().to_string(),
                source: source.to_string(),
                local: false,
            });
        }
    }
}

impl Editor {
    pub fn new(repo: &Path) -> Result<Self> {
        let repo = repo.canonicalize().map_err(|e| e.to_string())?;
        let root = repo
            .join("data/addon/data1.rda/addondata/config/gui")
            .canonicalize()
            .map_err(|e| format!("Extracted addon GUI tree missing: {e}"))?;
        let mut docs = BTreeMap::new();
        let mut errors = vec![];
        walk(&root, &root, &mut docs, &mut errors)?;
        let mut templates: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (key, xml) in &docs {
            if key.ends_with(".tpl") {
                templates
                    .entry(text(parse(xml)?.root_element(), "ACCESSID"))
                    .or_default()
                    .push(key.clone());
            }
        }
        let mut editor = Self {
            workspace: repo.join("tools/gui_editor/workspaces/default"),
            import_root: repo.clone(),
            repo,
            root,
            docs,
            overrides: BTreeMap::new(),
            templates,
            errors,
            session: None,
            serial: 0,
        };
        fs::create_dir_all(&editor.workspace).map_err(|e|e.to_string())?;
        editor.load_import_sources()?;
        editor.refresh_templates()?;
        Ok(editor)
    }
    pub fn with_workspace(repo: &Path, workspace: &Path) -> Result<Self> {
        let mut editor = Self::new(repo)?;
        fs::create_dir_all(workspace).map_err(|e|e.to_string())?;
        editor.workspace = workspace.canonicalize().map_err(|e|e.to_string())?;
        if let Ok(source_repo) = discover_repo() { editor.import_root = source_repo; }
        editor.docs.retain(|key, _| !key.starts_with("imports/"));
        editor.overrides.clear();
        fs::create_dir_all(&editor.workspace).map_err(|e|e.to_string())?;
        editor.load_import_sources()?;
        editor.refresh_templates()?;
        Ok(editor)
    }
    fn load_import_sources(&mut self)->Result<()> {
        let source_root=self.workspace.join("import_sources");
        fs::create_dir_all(&source_root).map_err(|e|e.to_string())?;
        let mut imported=BTreeMap::new();let mut errors=vec![];
        walk(&source_root,&source_root,&mut imported,&mut errors)?;
        for (relative,xml) in imported {
            let key=format!("imports/{relative}");
            self.docs.insert(key,xml);
        }
        self.errors.extend(errors.into_iter().map(|e|format!("import source: {e}")));
        Ok(())
    }
    fn refresh_templates(&mut self) -> Result<()> {
        let mut overrides = BTreeMap::new();
        let mut templates: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (key, original) in &self.docs {
            if !key.ends_with(".tpl") {
                continue;
            }
            let path = self.output_path(key, false)?;
            let xml = if path.is_file() {
                let xml = fs::read_to_string(path).map_err(|e| e.to_string())?;
                parse(&xml)?;
                overrides.insert(key.clone(), xml.clone());
                xml
            } else {
                original.clone()
            };
            templates
                .entry(text(parse(&xml)?.root_element(), "ACCESSID"))
                .or_default()
                .push(key.clone());
        }
        self.overrides = overrides;
        self.templates = templates;
        Ok(())
    }
    pub fn catalog(&self) -> CatalogInfo {
        let mut counts = BTreeMap::new();
        for key in self.docs.keys() {
            *counts
                .entry(key.rsplit('.').next().unwrap().to_string())
                .or_insert(0) += 1;
        }
        CatalogInfo {
            files: self.docs.keys().cloned().collect(),
            counts,
            repo: self.repo.display().to_string(),
            errors: self.errors.clone(),
        }
    }
    fn layers(
        &self,
        n: Node,
        source: &str,
        chain: &mut Vec<String>,
        out: &mut Vec<Vec<Property>>,
        warnings: &mut Vec<String>,
        handlers: &mut Vec<Handler>,
    ) {
        let id = text(n, "TEMPLATE_REF_ID");
        if !id.is_empty() {
            if chain.contains(&id) {
                warnings.push(format!("Template cycle: {id}"));
            } else if let Some(keys) = self.templates.get(&id).filter(|x| x.len() == 1) {
                if chain.len() >= 100 {
                    warnings.push("Template chain depth limit".into());
                } else {
                    let key = &keys[0];
                    if let Some(xml) = self.overrides.get(key).or_else(|| self.docs.get(key)) {
                        if let Ok(d) = parse(xml) {
                            chain.push(id);
                            self.layers(d.root_element(), key, chain, out, warnings, handlers);
                            chain.pop();
                        }
                    }
                }
            } else {
                warnings.push(format!("Missing/ambiguous template: {id}"));
            }
        }
        let mut props = vec![];
        collect(n, source, "", &mut props);
        out.push(props);
        // 00DC2A90 keeps the first definition for an event name. Preserve each
        // action in XML order instead of merging repeated property leaf keys.
        for h in children(n).filter(|n|n.has_tag_name("EVENTHANDLER")) {
            let event=text(h,"EVENT");
            if handlers.iter().any(|h|h.event==event) {warnings.push(format!("Event {event} in {source} ignored: an earlier handler already defines it"));continue}
            let actions=children(h).filter(|a|!a.has_tag_name("EVENT")).map(|a|Property{key:a.tag_name().name().into(),value:a.text().unwrap_or("").trim().into(),source:source.into(),local:false}).collect();
            handlers.push(Handler{event,source:source.into(),actions});
        }
    }
    fn tree(&self, n: Node, source: &str, id: String) -> Widget {
        let mut layers = vec![];
        let mut warnings = vec![];
        let mut handlers=vec![];
        self.layers(n, source, &mut vec![], &mut layers, &mut warnings,&mut handlers);
        let mut values = BTreeMap::new();
        if n.has_tag_name("FRAME") {
            for key in ["ALIGNMENT_LEFTHORIZONTAL", "ALIGNMENT_RIGHTHORIZONTAL", "ALIGNMENT_TOPVERTICAL", "ALIGNMENT_BOTTOMVERTICAL"] {
                values.insert(key.into(), Property { key:key.into(), value:"0".into(), source:"engine:0x00D8BC20".into(), local:false });
            }
        }
        if n.has_tag_name("BOXLAYOUT") {
            for (key, value) in [
                ("LINE_ORDER", "1"),
                ("LINECOLUMNCOUNT", "1"),
                ("XSPACE", "0"),
                ("YSPACE", "0"),
                ("LEFT_TO_RIGHT", "1"),
                ("TOP_TO_BOTTOM", "1"),
                ("BOXLAYOUT_BEHAVIOUR", "0"),
            ] {
                values.insert(
                    key.to_string(),
                    Property {
                        key: key.into(),
                        value: value.into(),
                        source: "engine:0x00D8D9C0".into(),
                        local: false,
                    },
                );
            }
        }
        for (i, layer) in layers.iter().enumerate() {
            let mut seen = std::collections::BTreeSet::new();
            for p in layer {
                if !seen.insert(&p.key) {
                    warnings.push(format!(
                        "Duplicate {} in {}; per-type duplicate semantics unverified",
                        p.key, p.source
                    ));
                }
                let mut p = p.clone();
                p.local = i + 1 == layers.len();
                values.insert(p.key.clone(), p);
            }
        }
        for key in ["PATHID", "ACCESSID", "TEMPLATE_REF_ID"] {
            values.insert(
                key.to_string(),
                Property {
                    key: key.into(),
                    value: text(n, key),
                    source: source.into(),
                    local: true,
                },
            );
        }
        let nodes = children(n)
            .filter(|x| widget(*x))
            .enumerate()
            .map(|(i, c)| self.tree(c, source, format!("{id}/{i}")))
            .collect();
        Widget {
            id,
            kind: n.tag_name().name().into(),
            name: text(n, "PATHID"),
            properties: values.into_values().collect(),
            layers,
            warnings,
            children: nodes,
            handlers,
        }
    }
    pub fn open(&mut self, key: &str) -> Result<View> {
        let original = self
            .docs
            .get(key)
            .ok_or("Definition is not in catalog")?
            .clone();
        let target = self.output_path(key, false)?;
        let disk = if target.is_file() {
            let xml = fs::read_to_string(&target).map_err(|e| e.to_string())?;
            parse(&xml)?;
            Some(xml)
        } else {
            None
        };
        let xml = disk.clone().unwrap_or_else(|| original.clone());
        self.serial += 1;
        self.session = Some(Session {
            key: key.into(),
            xml,
            original,
            disk,
            undo: vec![],
            redo: vec![],
            revision: self.serial,
            saved_to: target.is_file().then(|| target.display().to_string()),
        });
        self.view()
    }
    /// Imports an XML definition into a private source snapshot, then
    /// opens it with saves routed to the editor workspace's separate working copy.
    pub fn import_definition(&mut self, relative:&str)->Result<View>{
        let (source,relative)=if Path::new(relative).is_absolute(){
            let source=Path::new(relative).canonicalize().map_err(|e|format!("Cannot read selected file: {e}"))?;
            if !source.is_file()||!matches!(source.extension().and_then(|v|v.to_str()),Some("dlg"|"tpl")){return Err("Select a .dlg or .tpl XML definition file".into())}
            let extension=source.extension().unwrap().to_string_lossy();
            let identity=source.to_string_lossy().to_lowercase();
            let digest=format!("{:x}",Sha256::digest(identity.as_bytes()));
            let stem=source.file_stem().and_then(|v|v.to_str()).unwrap_or("dialog");
            let stem:String=stem.chars().take(64).map(|c|if c.is_ascii_alphanumeric()||matches!(c,'-'|'_'|'.'){c}else{'_'}).collect();
            let name=format!("selected/{}-{}.{extension}",if stem.is_empty(){"dialog"}else{&stem},&digest[..12]);
            (source,name)
        }else{
            let relative=import_key(relative)?;
            let source=self.import_root.join(&relative).canonicalize().map_err(|e|format!("Cannot read repo file: {e}"))?;
            if !source.starts_with(&self.import_root)||!source.is_file(){return Err("Import source must be a file inside this repo".into())}
            (source,relative)
        };
        let logical=Path::new(&relative);
        let bytes=fs::read(&source).map_err(|e|e.to_string())?;
        let xml=std::str::from_utf8(&bytes).map_err(|e|format!("Definition must be UTF-8: {e}"))?;
        parse(xml)?;
        let snapshot_root=self.workspace.join("import_sources");
        fs::create_dir_all(&snapshot_root).map_err(|e|e.to_string())?;
        let snapshot=snapshot_root.join(logical);
        if snapshot.exists(){
            if fs::read(&snapshot).map_err(|e|e.to_string())?!=bytes{return Err("This path was already imported with different source bytes; the saved source snapshot was preserved".into())}
        }else{
            if let Some(parent)=snapshot.parent(){fs::create_dir_all(parent).map_err(|e|e.to_string())?}
            atomic_write(&snapshot,&bytes)?;
        }
        let key=format!("imports/{}",relative.replace('\\',"/"));
        self.docs.insert(key.clone(),xml.to_string());
        self.refresh_templates()?;
        self.open(&key)
    }
    pub fn view(&self) -> Result<View> {
        let s = self.session.as_ref().ok_or("No open definition")?;
        let doc = parse(&s.xml)?;
        let n = doc.root_element();
        let bkg = n.tag_name().name() == "BackgroundLayout";
        let states = if bkg {
            child(n, "BackgroundInfo")
                .map(|x| children(x).map(|i| text(i, "LayoutName")).collect())
                .unwrap_or_default()
        } else {
            vec![]
        };
        Ok(View {
            key: s.key.clone(),
            xml: s.xml.clone(),
            original: s.original.clone(),
            revision: s.revision,
            dirty: &s.xml != s.disk.as_ref().unwrap_or(&s.original),
            can_undo: !s.undo.is_empty(),
            can_redo: !s.redo.is_empty(),
            tree: (!bkg).then(|| self.tree(n, &s.key, "0".into())),
            states,
            preview: if bkg { self.asset(&s.key).ok() } else { None },
            saved_to: s.saved_to.clone(),
        })
    }
    pub fn asset(&self, key: &str) -> Result<String> {
        if !key.ends_with(".bkg") || !self.docs.contains_key(key) {
            return Err("Background not in catalog".into());
        }
        let p = self
            .root
            .join(format!("{key}.png"))
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if !p.starts_with(&self.root) {
            return Err("Asset outside source root".into());
        }
        let b = fs::read(p).map_err(|e| e.to_string())?;
        if !b.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("Preview is not PNG".into());
        }
        Ok(format!("data:image/png;base64,{}", STANDARD.encode(b)))
    }
    pub fn background(
        &self,
        key: &str,
        state: &str,
        width: i32,
        height: i32,
    ) -> Result<crate::background::Background> {
        if !key.ends_with(".bkg") {
            return Err("Expected background definition".into());
        }
        let xml = self.docs.get(key).ok_or("Background not in catalog")?;
        crate::background::render_data(&self.repo, xml, state, width, height)
    }
    pub fn gallery_background(&self,key:&str,archive:Option<&str>)->Result<crate::background::Background>{
        if !key.ends_with(".bkg"){return Err("Expected background definition".into())}
        let xml=self.docs.get(key).ok_or("Background not in catalog")?;
        let doc=parse(xml)?;
        let layouts=child(doc.root_element(),"BackgroundInfo").ok_or("Missing BackgroundInfo")?;
        let state=children(layouts).map(|n|text(n,"LayoutName")).find(|s|!s.is_empty()).ok_or("No named background states")?;
        crate::background::render_data_candidate(&self.repo,xml,&state,240,120,archive)
    }
    pub fn background_candidate(&self,key:&str,state:&str,width:i32,height:i32,archive:&str)->Result<crate::background::Background>{
        if !key.ends_with(".bkg"){return Err("Expected background definition".into())}
        let xml=self.docs.get(key).ok_or("Background not in catalog")?;
        crate::background::render_data_candidate(&self.repo,xml,state,width,height,Some(archive))
    }
    pub fn preview_progress(&self,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,visibility:&BTreeMap<String,bool>,buttons:&BTreeMap<String,crate::button_state::Input>,inputs:&BTreeMap<String,crate::progress::Input>)->Result<crate::progress::Render>{
        self.preview_progress_candidate(revision,width,height,show_hidden,cards,visibility,buttons,inputs,None)
    }
    pub fn preview_progress_candidate(&self,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,visibility:&BTreeMap<String,bool>,buttons:&BTreeMap<String,crate::button_state::Input>,inputs:&BTreeMap<String,crate::progress::Input>,archive:Option<&str>)->Result<crate::progress::Render>{
        self.preview_progress_text_scenario(revision,width,height,show_hidden,cards,visibility,buttons,inputs,archive,"eng",&crate::captions::Runtime::default())
    }
    pub fn preview_progress_text_scenario(&self,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,visibility:&BTreeMap<String,bool>,buttons:&BTreeMap<String,crate::button_state::Input>,inputs:&BTreeMap<String,crate::progress::Input>,archive:Option<&str>,language:&str,runtime:&crate::captions::Runtime)->Result<crate::progress::Render>{
        let preview=self.preview_layout_text_scenario(revision,width,height,show_hidden,cards,visibility,buttons,language,runtime)?;
        Ok(crate::progress::render_candidate(&self.repo,&self.docs,&self.view()?.tree.ok_or("No tree")?,&preview,inputs,archive))
    }
    pub fn preview_layout(
        &self,
        revision: u64,
        width: i32,
        height: i32,
        show_hidden: bool,
        cards: &BTreeMap<String, usize>,
    ) -> Result<crate::layout::Preview> {
        self.preview_layout_scenario(revision,width,height,show_hidden,cards,&BTreeMap::new())
    }
    pub fn preview_layout_scenario(&self,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,visibility:&BTreeMap<String,bool>)->Result<crate::layout::Preview>{
        self.preview_layout_visual_scenario(revision,width,height,show_hidden,cards,visibility,&BTreeMap::new())
    }
    pub fn preview_layout_visual_scenario(&self,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,visibility:&BTreeMap<String,bool>,buttons:&BTreeMap<String,crate::button_state::Input>)->Result<crate::layout::Preview>{
        self.preview_layout_text_scenario(revision,width,height,show_hidden,cards,visibility,buttons,"eng",&crate::captions::Runtime::default())
    }
    pub fn preview_layout_text_scenario(&self,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,visibility:&BTreeMap<String,bool>,buttons:&BTreeMap<String,crate::button_state::Input>,language:&str,runtime:&crate::captions::Runtime)->Result<crate::layout::Preview>{
        self.check(revision)?;
        runtime.validate()?;
        if !matches!(language,"eng"|"ger"){return Err("Choose an extracted language".into())}
        if !(1..=4096).contains(&width) || !(1..=4096).contains(&height) {
            return Err("Invalid preview viewport".into());
        }
        let tree = self.view()?.tree.ok_or("No widget tree")?;
        fn text_owner<'a>(node:&'a Widget,id:&str)->Option<&'a Widget>{if node.id==id{Some(node)}else{node.children.iter().find_map(|child|text_owner(child,id))}}
        for id in runtime.text_refresh.keys(){let node=text_owner(&tree,id).ok_or_else(||format!("Text refresh widget {id} is not in this definition"))?;if node.kind!="LABEL"||!node.properties.iter().any(|p|p.key=="TEXTSETTING/TEXTGUID"){return Err(format!("Text refresh widget {id} needs a source LABEL caption"))}}
        let mut offsets = BTreeMap::new();
        let mut selections=BTreeMap::new();let mut diagnostics=vec![];
        fn gather(editor: &Editor, node: &Widget, parent_enabled:bool,buttons:&BTreeMap<String,crate::button_state::Input>,offsets: &mut BTreeMap<String, [i32; 4]>,selections:&mut BTreeMap<String,crate::button_state::Selection>,diagnostics:&mut Vec<String>) {
            let input=buttons.get(&node.id).cloned().unwrap_or_default();
            let source_enabled=!node.properties.iter().any(|p|p.key=="DISABLE"&&p.value.parse::<i32>().unwrap_or(0)!=0);
            let enabled=parent_enabled&&input.enabled.unwrap_or(source_enabled);
            if let Some(name) = node.properties.iter().find(|p| p.key == "BACKGROUND") {
                let normalized = name.value.replace('\\', "/");
                if let Some((_, key)) = normalized.split_once("/config/gui/") {
                    if let Some(xml) = editor.docs.get(key) {
                        let state=if node.kind=="BUTTON" {
                            match crate::button_state::select(xml,&input,enabled){Ok(selection)=>{
                                if selection.fallback{diagnostics.push(format!("{}: {} missing; recovered button fallback selects first layout {}",node.id,selection.requested,selection.selected));}
                                let name=selection.selected.clone();selections.insert(node.id.clone(),selection);name
                            },Err(e)=>{diagnostics.push(format!("{}: {e}",node.id));String::new()}}
                        }else if node.kind=="PROGRESSBAR"{
                            match crate::progress::names(xml){Ok(names)=>{
                                match names.iter().enumerate().find(|(_,n)|n.eq_ignore_ascii_case("PB_Background")){
                                    Some((index,name))=>{selections.insert(node.id.clone(),crate::button_state::Selection{requested:"PB_Background".into(),selected:name.clone(),index,fallback:false});name.clone()},
                                    None=>{diagnostics.push(format!("{}: PB_Background missing; active-layout history unresolved",node.id));String::new()}
                                }
                            },Err(e)=>{diagnostics.push(format!("{}: {e}",node.id));String::new()}}
                        }else{"EmptyLayout".into()};
                        if let Ok(data) = crate::background::grid(xml, &state, 100, 100) {
                            offsets.insert(node.id.clone(), data.client_offsets);
                        }
                    }
                }
            }
            for child in &node.children {
                gather(editor, child, enabled,buttons,offsets,selections,diagnostics);
            }
        }
        gather(self, &tree,true,buttons,&mut offsets,&mut selections,&mut diagnostics);
        let(text_sizes,text_diagnostics)=crate::captions::intrinsic_sizes(&self.repo,language,&tree,buttons,runtime,&offsets);
        diagnostics.extend(text_diagnostics);
        runtime.layout.validate_tree(&tree,width,height,&offsets)?;
        let mut preview=crate::layout::preview_scene(
            &tree,
            revision,
            width,
            height,
            show_hidden,
            cards,
            &offsets,
            visibility,
            &text_sizes,
            &runtime.layout,
        );
        preview.background_states=selections;
        preview.diagnostics.extend(diagnostics);
        preview.diagnostics.push("BUTTON background variants use explicit hover/press/check/enabled inputs and source DISABLE ancestry. Runtime hover suppression, forced states, keyboard focus overlay, tint and text colors remain outside this preview.".into());
        Ok(preview)
    }
    fn check(&self, revision: u64) -> Result<()> {
        if self.session.as_ref().ok_or("No open definition")?.revision != revision {
            return Err("Stale edit: refresh document".into());
        }
        Ok(())
    }
    pub fn edit(
        &mut self,
        revision: u64,
        id: &str,
        key: &str,
        value: Option<&str>,
    ) -> Result<View> {
        self.edit_many(
            revision,
            id,
            &[(key.to_string(), value.map(str::to_string))],
        )
    }
    pub fn edit_many(
        &mut self,
        revision: u64,
        id: &str,
        edits: &[(String, Option<String>)],
    ) -> Result<View> {
        self.check(revision)?;
        if edits.len() > 16 {
            return Err("Too many edits".into());
        }
        let mut xml = self.session.as_ref().unwrap().xml.clone();
        for (key, value) in edits {
            xml = patch(&xml, id, key, value.as_deref())?;
        }
        parse(&xml)?;
        let s = self.session.as_mut().unwrap();
        if xml != s.xml {
            s.undo.push(s.xml.clone());
            s.redo.clear();
            s.xml = xml;
            self.serial += 1;
            s.revision = self.serial;
        }
        self.view()
    }
    pub fn history(&mut self, revision: u64, redo: bool) -> Result<View> {
        self.check(revision)?;
        let s = self.session.as_mut().unwrap();
        let next = if redo { s.redo.pop() } else { s.undo.pop() }.ok_or("History empty")?;
        if redo {
            s.undo.push(s.xml.clone());
        } else {
            s.redo.push(s.xml.clone());
        }
        s.xml = next;
        self.serial += 1;
        s.revision = self.serial;
        self.view()
    }
    fn output_path(&self, key: &str, create: bool) -> Result<PathBuf> {
        if !self.docs.contains_key(key) {
            return Err("Unknown output definition".into());
        }
        let relative = Path::new(key);
        if relative
            .components()
            .any(|x| !matches!(x, std::path::Component::Normal(_)))
        {
            return Err("Invalid logical path".into());
        }
        let workspace = &self.workspace;
        let target = workspace.join(relative);
        let mut ancestor = target.as_path();
        while !ancestor.exists() { ancestor = ancestor.parent().ok_or("Invalid output path")?; }
        let canonical = ancestor.canonicalize().map_err(|e|e.to_string())?;
        let workspace_root = workspace.canonicalize().map_err(|e|e.to_string())?;
        if !canonical.starts_with(&workspace_root) { return Err("Working copy path redirected outside editor".into()); }
        if create {
            fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        }
        Ok(target)
    }
    pub fn save(&mut self, revision: u64) -> Result<View> {
        self.check(revision)?;
        let s = self.session.as_ref().unwrap();
        let path = self.output_path(&s.key, true)?;
        let disk = if path.exists() {
            Some(fs::read_to_string(&path).map_err(|e| e.to_string())?)
        } else {
            None
        };
        if disk != s.disk {
            return Err("Working copy changed externally; reopen before saving".into());
        }
        let source_path = if let Some(relative) = s.key.strip_prefix("imports/") {
            let source_root = self.workspace.join("import_sources").canonicalize().map_err(|e| e.to_string())?;
            let source_path = source_root.join(relative).canonicalize().map_err(|e| e.to_string())?;
            if !source_path.starts_with(&source_root) {
                return Err("Imported source redirected outside its private snapshot".into());
            }
            source_path
        } else {
            let source_path = self.root.join(&s.key).canonicalize().map_err(|e| e.to_string())?;
            if !source_path.starts_with(&self.root) {
                return Err("Original source redirected outside the asset root".into());
            }
            source_path
        };
        if fs::read_to_string(source_path).map_err(|e| e.to_string())? != s.original {
            return Err("Original source changed externally; reopen editor".into());
        }
        let manifest_path = self.workspace.join("project.json");
        let workspace = manifest_path
            .parent()
            .unwrap()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if manifest_path.exists()
            && !manifest_path
                .canonicalize()
                .map_err(|e| e.to_string())?
                .starts_with(&workspace)
        {
            return Err("Manifest redirected outside working directory".into());
        }
        let mut manifest = if manifest_path.exists() {
            serde_json::from_slice::<serde_json::Value>(
                &fs::read(&manifest_path).map_err(|e| e.to_string())?,
            )
            .map_err(|e| format!("Invalid project manifest: {e}"))?
        } else {
            serde_json::json!({"version":1,"source_root":self.root,"files":{}})
        };
        if !manifest["files"].is_object() {
            return Err("Invalid project manifest files".into());
        }
        manifest["files"][&s.key] = serde_json::json!({"source_sha256":hash(s.original.as_bytes()),"working_sha256":hash(s.xml.as_bytes())});
        atomic_write(&path, s.xml.as_bytes())?;
        atomic_write(
            &manifest_path,
            &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
        )?;
        let s = self.session.as_mut().unwrap();
        s.disk = Some(s.xml.clone());
        s.saved_to = Some(path.display().to_string());
        self.refresh_templates()?;
        self.view()
    }
    pub fn audit(&self) -> Result<serde_json::Value> {
        let mut refs = 0;
        let mut unresolved = vec![];
        let mut cycles = vec![];
        for (key, xml) in &self.docs {
            let d = parse(xml)?;
            for n in d
                .descendants()
                .filter(|n| n.is_element() && n.tag_name().name() == "TEMPLATE_REF_ID")
            {
                let id = n.text().unwrap_or("").trim();
                if id.is_empty() {
                    continue;
                }
                refs += 1;
                if !self.templates.get(id).is_some_and(|x| x.len() == 1) {
                    unresolved.push(format!("{key}: {id}"));
                }
            }
            if key.ends_with(".tpl") {
                let mut warnings = vec![];
                self.layers(
                    d.root_element(),
                    key,
                    &mut vec![],
                    &mut vec![],
                    &mut warnings,
                    &mut vec![],
                );
                cycles.extend(warnings);
            }
        }
        Ok(
            serde_json::json!({"counts": self.catalog().counts, "references": refs, "unresolved": unresolved, "cycles": cycles, "errors": self.errors}),
        )
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let temp = path.with_extension(format!("annore-{}.tmp", std::process::id()));
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        f.write_all(bytes).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
        drop(f);
        fs::rename(&temp, path).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

fn find_widget<'a, 'input>(root: Node<'a, 'input>, id: &str) -> Result<Node<'a, 'input>> {
    let mut segments = id.split('/');
    if segments.next() != Some("0") {
        return Err("Invalid node ID".into());
    }
    let mut n = root;
    for part in segments {
        let i: usize = part.parse().map_err(|_| "Invalid node ID")?;
        n = children(n)
            .filter(|x| widget(*x))
            .nth(i)
            .ok_or("Node no longer exists")?;
    }
    Ok(n)
}

/// Only explicitly editable scalars. Unknown XML stays inspectable, never rewritten.
pub fn editable(key: &str) -> bool {
    matches!(
        key,
        "PATHID"
            | "ACCESSID"
            | "TEMPLATE_REF_ID"
            | "BACKGROUND"
            | "HIDDEN"
            | "TRANSPARENT"
            | "ZORDER"
            | "ALPHA"
            | "ICONGUID"
            | "ICONSTRETCH"
            | "EXTENT/LEFT"
            | "EXTENT/TOP"
            | "EXTENT/WIDTH"
            | "EXTENT/HEIGHT"
            | "XSPACE"
            | "YSPACE"
            | "LINE_ORDER"
            | "LINECOLUMNCOUNT"
            | "TEXTSETTING/TEXTGUID"
            | "TEXTSETTING/FONT"
    )
}
pub fn patch(xml: &str, id: &str, key: &str, value: Option<&str>) -> Result<String> {
    if !editable(key) {
        return Err("Property is inspection-only in this version".into());
    }
    if let Some(v) = value {
        if v.len() > 1024 || v.chars().any(|x| x.is_control()) {
            return Err("Invalid scalar value".into());
        }
        if key.starts_with("EXTENT/")
            || matches!(
                key,
                "ZORDER"
                    | "XSPACE"
                    | "YSPACE"
                    | "LINE_ORDER"
                    | "LINECOLUMNCOUNT"
                    | "HIDDEN"
                    | "TRANSPARENT"
                    | "ICONGUID"
                    | "ICONSTRETCH"
                    | "TEXTSETTING/TEXTGUID"
            )
        {
            if !v.is_empty() {
                let number = v.parse::<i32>().map_err(|_| "Expected a 32-bit integer")?;
                if matches!(key, "EXTENT/WIDTH" | "EXTENT/HEIGHT") && number < 0 {
                    return Err("Size cannot be negative".into());
                }
            }
        }
        if key == "ALPHA" && !v.is_empty() {
            let n = v.parse::<f64>().map_err(|_| "Expected alpha 0..1")?;
            if !(0.0..=1.0).contains(&n) {
                return Err("Expected alpha 0..1".into());
            }
        }
    }
    let doc = parse(xml)?;
    let mut n = find_widget(doc.root_element(), id)?;
    if n.tag_name().name() == "BackgroundLayout" {
        return Err("Background layouts are read-only".into());
    }
    let parts: Vec<&str> = key.split('/').collect();
    for (i, tag) in parts.iter().enumerate() {
        let found: Vec<_> = children(n)
            .filter(|c| c.tag_name().name() == *tag)
            .collect();
        if found.len() > 1 {
            return Err("Duplicate property: use source inspection before editing".into());
        }
        if let Some(c) = found.first().copied() {
            if i + 1 == parts.len() {
                if children(c).next().is_some() || c.children().any(|x| x.is_comment() || x.is_pi())
                {
                    return Err("Scalar contains unknown content; inspection-only".into());
                }
                let range = c.range();
                let slice = &xml[range.clone()];
                let replacement = if let Some(v) = value {
                    if slice.trim_end().ends_with("/>") {
                        format!(
                            "{}>{}</{}>",
                            slice.trim_end().trim_end_matches("/>").trim_end(),
                            escape(v),
                            tag
                        )
                    } else {
                        let a = opening_end(slice)?;
                        let b = slice.rfind("</").ok_or("Invalid scalar")?;
                        let content = &slice[a..b];
                        let leading = &content[..content.len() - content.trim_start().len()];
                        let trailing = if content.trim().is_empty() {
                            ""
                        } else {
                            &content[content.trim_end().len()..]
                        };
                        format!(
                            "{}{}{}{}{}",
                            &slice[..a],
                            leading,
                            escape(v),
                            trailing,
                            &slice[b..]
                        )
                    }
                } else {
                    String::new()
                };
                let mut output = xml.to_string();
                output.replace_range(range, &replacement);
                parse(&output)?;
                return Ok(output);
            }
            n = c;
        } else {
            let Some(v) = value else {
                return Ok(xml.to_string());
            };
            let mut insert = escape(v);
            for part in parts[i..].iter().rev() {
                insert = format!("<{part}>{insert}</{part}>");
            }
            let range = n.range();
            let slice = &xml[range.clone()];
            let mut output = xml.to_string();
            if slice.trim_end().ends_with("/>") {
                let replacement = format!(
                    "{}>{}</{}>",
                    slice.trim_end().trim_end_matches("/>").trim_end(),
                    insert,
                    n.tag_name().name()
                );
                output.replace_range(range, &replacement);
            } else {
                let at = range.start + slice.rfind("</").ok_or("Invalid container")?;
                output.insert_str(at, &insert);
            }
            parse(&output)?;
            return Ok(output);
        }
    }
    Err("Invalid property path".into())
}

pub fn discover_repo() -> Result<PathBuf> {
    let candidates = [
        std::env::current_dir().ok(),
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf)),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR"))),
    ];
    for p in candidates.into_iter().flatten() {
        for root in p.ancestors() {
            if root.join("AGENTS.md").is_file()
                && root
                    .join("data/addon/data1.rda/addondata/config/gui")
                    .is_dir()
            {
                return Ok(root.to_path_buf());
            }
        }
    }
    Err("Run the editor from the Anno1404-RE repository containing extracted GUI data".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_paths_are_repo_relative_definition_paths() {
        assert_eq!(import_key(r"tools\production-menu\examples\production_dialog_v5.dlg").unwrap(), "tools/production-menu/examples/production_dialog_v5.dlg");
        assert!(import_key("../outside.dlg").is_err());
        assert!(import_key("tools/../outside.dlg").is_err());
        assert!(import_key("/outside.dlg").is_err());
        assert!(import_key("C:/outside.dlg").is_err());
        assert!(import_key("tools/production-menu/README.md").is_err());
        assert!(import_key("").is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn button_scenario_offsets_change_children_and_parent_disable_wins(){
        let repo=discover_repo().unwrap();let mut editor=Editor::new(&repo).unwrap();
        let xml="<FRAME><DISABLE>0</DISABLE><EXTENT><WIDTH>100</WIDTH><HEIGHT>100</HEIGHT></EXTENT><BUTTON><EXTENT><WIDTH>80</WIDTH><HEIGHT>60</HEIGHT></EXTENT><BACKGROUND>addondata/config/gui/backgrounds/state_test.bkg</BACKGROUND><LABEL><EXTENT><WIDTH>5</WIDTH><HEIGHT>5</HEIGHT></EXTENT></LABEL></BUTTON></FRAME>";
        let layout=|name:&str,left:i32|format!("<i><LayoutName>{name}</LayoutName><flags>0</flags><m_XSize>1</m_XSize><m_YSize>1</m_YSize><ClientAreaOffsets><left>{left}</left><top>0</top><right>0</right><bottom>0</bottom></ClientAreaOffsets><IconList><i><isOldFormat>0</isOldFormat><flags>0</flags><tileX>0</tileX><tileY>0</tileY><tileWidth>0</tileWidth><tileHeight>0</tileHeight><LocalFileID>-1</LocalFileID><Width>0</Width><Height>0</Height></i></IconList></i>");
        let background=format!("<BackgroundLayout><m_localIconFileMap/><BackgroundInfo>{}{}{}</BackgroundInfo></BackgroundLayout>",layout("Normal",3),layout("Normal+MouseOver",11),layout("Normal+Disabled",17));
        editor.docs.insert("backgrounds/state_test.bkg".into(),background);
        editor.docs.insert("menudefinitions/state_test.dlg".into(),xml.into());
        let view=editor.open("menudefinitions/state_test.dlg").unwrap();
        let cards=BTreeMap::new();let visibility=BTreeMap::new();let normal=editor.preview_layout_visual_scenario(view.revision,1280,1024,false,&cards,&visibility,&BTreeMap::new()).unwrap();
        assert_eq!(normal.rects.iter().find(|r|r.id=="0/0/0").unwrap().x,3);
        let buttons=BTreeMap::from([("0/0".into(),crate::button_state::Input{hovered:true,enabled:Some(true),..Default::default()})]);
        let hover=editor.preview_layout_visual_scenario(view.revision,1280,1024,false,&cards,&visibility,&buttons).unwrap();
        assert_eq!(hover.rects.iter().find(|r|r.id=="0/0/0").unwrap().x,11);
        assert_eq!(editor.view().unwrap().xml,xml); // Scenario is never an XML edit.
        editor.docs.insert("menudefinitions/state_test_disabled.dlg".into(),xml.replace("<DISABLE>0</DISABLE>","<DISABLE>1</DISABLE>"));
        let disabled=editor.open("menudefinitions/state_test_disabled.dlg").unwrap();
        let preview=editor.preview_layout_visual_scenario(disabled.revision,1280,1024,false,&cards,&visibility,&buttons).unwrap();
        assert_eq!(preview.background_states["0/0"].requested,"Normal+Disabled");
        assert_eq!(preview.rects.iter().find(|r|r.id=="0/0/0").unwrap().x,17);
    }
    #[test]
    fn preserve_unknown_bytes_and_bom() {
        let xml = "\u{feff}<FRAME a='b'>\r\n<!--x--><PATHID>A</PATHID><UNKNOWN weird='yes'/><EXTENT><LEFT> 3 </LEFT></EXTENT><LABEL/><LABEL/></FRAME>";
        let changed = patch(xml, "0", "EXTENT/LEFT", Some("7")).unwrap();
        assert_eq!(changed, xml.replace("> 3 <", "> 7 <"));
        assert_eq!(patch(xml, "0", "EXTENT/LEFT", Some("3")).unwrap(), xml);
        let inserted = patch(xml, "0/1", "EXTENT/WIDTH", Some("40")).unwrap();
        assert!(inserted.contains("<LABEL/><LABEL><EXTENT><WIDTH>40</WIDTH></EXTENT></LABEL>"));
        assert!(patch(xml, "0", "UNKNOWN", Some("bad")).is_err());
        assert_eq!(
            patch(
                "<FRAME><PATHID attr='>'>old</PATHID></FRAME>",
                "0",
                "PATHID",
                Some("new")
            )
            .unwrap(),
            "<FRAME><PATHID attr='>'>new</PATHID></FRAME>"
        );
    }
    #[test]
    fn reset_duplicates_and_validation() {
        assert_eq!(
            patch("<FRAME><HIDDEN>1</HIDDEN><X/></FRAME>", "0", "HIDDEN", None).unwrap(),
            "<FRAME><X/></FRAME>"
        );
        assert!(patch(
            "<FRAME><HIDDEN>1</HIDDEN><HIDDEN>0</HIDDEN></FRAME>",
            "0",
            "HIDDEN",
            Some("1")
        )
        .is_err());
        assert!(patch("<FRAME/>", "0", "EXTENT/WIDTH", Some("-1")).is_err());
        assert!(patch("<FRAME/>", "0", "ALPHA", Some("NaN")).is_err());
        assert!(patch(
            "<FRAME><BACKGROUND><!--keep--></BACKGROUND></FRAME>",
            "0",
            "BACKGROUND",
            Some("x")
        )
        .is_err());
    }
    #[test]
    fn imported_definition_edits_only_workspace_copy_and_reopens() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-fixtures")
            .join(format!("import-{}", std::process::id()));
        let source = fixture.join("bridge/production_dialog.dlg");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::create_dir_all(fixture.join("data/addon/data1.rda/addondata/config/gui")).unwrap();
        let original = "<FRAME><PATHID>ROOT</PATHID><EXTENT><WIDTH>100</WIDTH></EXTENT></FRAME>";
        fs::write(&source, original).unwrap();

        let mut editor = Editor::new(&fixture).unwrap();
        let opened = editor.import_definition(source.to_str().unwrap()).unwrap();
        assert_eq!(opened.tree.as_ref().unwrap().name, "ROOT");
        let key = opened.key.clone();
        let edited = editor
            .edit(opened.revision, "0", "EXTENT/WIDTH", Some("120"))
            .unwrap();
        let saved = editor.save(edited.revision).unwrap();
        let saved_to = PathBuf::from(saved.saved_to.unwrap());
        let expected_root = fixture
            .canonicalize()
            .unwrap()
            .join("tools/gui_editor/workspaces/default/imports/selected")
            .canonicalize()
            .unwrap();
        assert!(saved_to.canonicalize().unwrap().starts_with(expected_root));
        assert_eq!(fs::read_to_string(&source).unwrap(), original);

        let mut restarted = Editor::new(&fixture).unwrap();
        let reopened = restarted.open(&key).unwrap();
        assert!(reopened.xml.contains("<WIDTH>120</WIDTH>"));
        assert_eq!(fs::read_to_string(&source).unwrap(), original);
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn full_corpus_and_history() {
        let repo = discover_repo().unwrap();
        let mut e = Editor::new(&repo).unwrap();
        assert_eq!(e.docs.len(), 1873);
        let audit = e.audit().unwrap();
        assert_eq!(audit["references"], 8497);
        assert_eq!(audit["unresolved"], serde_json::json!([]));
        assert_eq!(audit["cycles"], serde_json::json!([]));
        assert!(e.errors.is_empty());
        let v = e.open("menudefinitions/infobar.dlg").unwrap();
        assert_eq!(v.tree.unwrap().name, "INFOBAR");
        let edit = e
            .edit(v.revision, "0", "EXTENT/WIDTH", Some("1200"))
            .unwrap();
        assert!(edit.dirty);
        assert!(e.edit(v.revision, "0", "HIDDEN", Some("1")).is_err());
        let undo = e.history(edit.revision, false).unwrap();
        assert_eq!(undo.xml, v.xml);
        let redo = e.history(undo.revision, true).unwrap();
        assert_eq!(redo.xml, edit.xml);
        assert!(e.open("../../AGENTS.md").is_err());
        assert!(e.asset("../../AGENTS.md").is_err());
        for (key, xml) in &e.docs {
            assert_eq!(
                hash(fs::read(e.root.join(key)).unwrap().as_slice()),
                hash(xml.as_bytes()),
                "{key}"
            );
        }
    }
    #[test]
    fn save_reopen_and_external_conflicts() {
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target/test-fixtures")
            .join(format!(
                "{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let source =
            fixture.join("data/addon/data1.rda/addondata/config/gui/menudefinitions/sample.dlg");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::create_dir_all(fixture.join("tools/gui_editor")).unwrap();
        let original = "\u{feff}<FRAME a='keep'>\r\n<!--preserve--><PATHID>ROOT</PATHID><EXTENT><WIDTH> 100 </WIDTH></EXTENT><UNKNOWN x='yes'/></FRAME>";
        fs::write(&source, original).unwrap();
        let mut e = Editor::new(&fixture).unwrap();
        let v = e.open("menudefinitions/sample.dlg").unwrap();
        let saved = e.save(v.revision).unwrap();
        let target = PathBuf::from(saved.saved_to.unwrap());
        assert_eq!(fs::read_to_string(&target).unwrap(), original);
        let v = e
            .edit(v.revision, "0", "EXTENT/WIDTH", Some("120"))
            .unwrap();
        e.save(v.revision).unwrap();
        let reopened = e.open("menudefinitions/sample.dlg").unwrap();
        assert!(reopened.xml.contains("> 120 <"));
        assert!(!reopened.dirty);
        assert_eq!(fs::read_to_string(&source).unwrap(), original);
        let v = e.edit(reopened.revision, "0", "HIDDEN", Some("1")).unwrap();
        fs::write(&target, "<FRAME/>").unwrap();
        assert!(e.save(v.revision).unwrap_err().contains("externally"));
        assert_eq!(fs::read_to_string(&target).unwrap(), "<FRAME/>");
    }
}
