//! Static unit-scale layout based on Addon.exe's pre/child/post pass.
//! Frame screen offsets are applied to accessor rectangles, never authored XML.
use crate::editor::Widget;
use serde::{Deserialize,Serialize};
use std::collections::BTreeMap;
#[derive(Clone, Debug, Serialize)]
pub struct Rect {
    pub id: String,
    pub parent: Option<String>,
    pub x: i32, pub y: i32, pub w: i32, pub h: i32,
    pub raw_relative: [i32;4],
    pub effective_relative: [i32;4],
    pub clip: [i32;4],
    pub client: [i32;4],
    pub alpha: f32,
    pub diagnostics: Vec<String>,
}
#[derive(Serialize)]
pub struct Preview {
    pub revision: u64,
    pub rects: Vec<Rect>,
    pub diagnostics: Vec<String>,
    pub background_states: BTreeMap<String,crate::button_state::Selection>,
    pub text_sizes:BTreeMap<String,TextSize>,
    pub passes:Vec<Pass>,
}
#[derive(Clone,Debug,Deserialize,Serialize)]#[serde(deny_unknown_fields)]
pub struct IslandInfoScenario{pub parent_viewport:[i32;2],pub wares_right:i32}
#[derive(Clone,Debug,Deserialize,Serialize)]#[serde(deny_unknown_fields)]
pub struct Scenario{#[serde(default="one_pass")]pub passes:u32,#[serde(default)]pub island_info:Option<IslandInfoScenario>}
fn one_pass()->u32{1}
impl Default for Scenario{fn default()->Self{Self{passes:1,island_info:None}}}
impl Scenario{
    pub fn validate(&self)->Result<(),String>{
        if !(1..=32).contains(&self.passes){return Err("Layout passes must be1..32".into())}
        if let Some(info)=&self.island_info{if info.parent_viewport.iter().any(|v|!(1..=4096).contains(v))||!(0..=4096).contains(&info.wares_right){return Err("Island positioning needs a1..4096 full-screen parent and wares right0..4096".into())}}
        Ok(())
    }
    pub fn validate_tree(&self,root:&Widget,width:i32,height:i32,offsets:&BTreeMap<String,[i32;4]>)->Result<(),String>{
        self.validate()?;
        if let Some(info)=&self.island_info{
            if root.kind!="FRAME"||root.name!="INFOBAR_ISLAND_INFO"||info.parent_viewport!=[width,height]||root.properties.iter().any(|p|p.key=="BACKGROUND"&&!p.value.is_empty())||offsets.get(&root.id).is_some_and(|o|*o!=[0;4])||["ALIGNMENT_LEFTHORIZONTAL","ALIGNMENT_RIGHTHORIZONTAL","ALIGNMENT_TOPVERTICAL","ALIGNMENT_BOTTOMVERTICAL"].iter().any(|k|n(root,k,0)!=1){return Err("Island positioning supports only the borderless INFOBAR_ISLAND_INFO frame with fixed edges and explicit full-viewport parent".into())}
            if !root.handlers.iter().any(|h|h.event=="OnRender"&&h.actions.iter().any(|a|a.key=="UPDATE_INFOBAR_ISLAND_INFO")){return Err("Island positioning requires the source UPDATE_INFOBAR_ISLAND_INFO handler".into())}
        }
        Ok(())
    }
}
#[derive(Serialize)]pub struct Pass{pub index:u32,pub root_raw:[i32;4],pub identical_to_previous:bool}
#[derive(Clone,Debug,Serialize)]pub struct TextSize{pub client:[i32;2],pub far_anchor:bool,pub pre_refresh_width:Option<i32>}
fn n(w: &Widget, key: &str, fallback: i32) -> i32 {
    w.properties.iter().find(|p| p.key == key).map(|p| {
        if p.value.is_empty() { 0 } else { p.value.parse().unwrap_or(fallback) }
    }).unwrap_or(fallback)
}
fn axis(position: i32, size: i32, available: i32, mode: i32) -> (i32,i32) {
    match mode {1=>(0,available),2=>(0,size),3=>(available/2-size/2,size),4=>(available-size,size),_=>(position,size)}
}
fn intersect(a:[i32;4],b:[i32;4])->[i32;4] { [a[0].max(b[0]),a[1].max(b[1]),a[2].min(b[2]),a[3].min(b[3])] }
struct Geometry<'a> { node:&'a Widget, raw:[i32;4], visible:bool, children:Vec<Geometry<'a>> }
impl<'a> Geometry<'a> {
    fn new(node:&'a Widget,visibility:&BTreeMap<String,bool>,offsets:&BTreeMap<String,[i32;4]>,text_sizes:&BTreeMap<String,TextSize>)->Self {
        let (x,y)=(n(node,"EXTENT/LEFT",0),n(node,"EXTENT/TOP",0));
        let mut children:Vec<_>=node.children.iter().map(|c|Self::new(c,visibility,offsets,text_sizes)).collect();
        children.sort_by_key(|c|n(c.node,"ZORDER",0));
        let mut raw=[x,y,x+n(node,"EXTENT/WIDTH",0),y+n(node,"EXTENT/HEIGHT",0)];
        if let Some(text)=text_sizes.get(&node.id){
            // D8B680 -> D5D770 runs before relayout. Grow measured client size
            // by the active background offsets and retain both near or far edges.
            let o=offsets.get(&node.id).copied().unwrap_or([0;4]);
            // Explicit refresh scenarios replace the initial client width,
            // keeping authored near position/height before the native anchor.
            if let Some(width)=text.pre_refresh_width{raw[2]=raw[0]+width+o[0]+o[2];}
            let size=[text.client[0].max(1)+o[0]+o[2],text.client[1].max(1)+o[1]+o[3]];
            if text.far_anchor{raw[0]=raw[2]-size[0];raw[1]=raw[3]-size[1]}else{raw[2]=raw[0]+size[0];raw[3]=raw[1]+size[1]}
        }
        Self{node,raw,visible:visibility.get(&node.id).copied().unwrap_or(n(node,"HIDDEN",0)==0),children}
    }
    fn effective(&self, viewport:[i32;2])->[i32;4] {
        let mut r=self.raw;
        if self.node.kind=="FRAME" {
            for (i,key) in ["ALIGNMENT_LEFTHORIZONTAL","ALIGNMENT_TOPVERTICAL","ALIGNMENT_RIGHTHORIZONTAL","ALIGNMENT_BOTTOMVERTICAL"].iter().enumerate() {
                let delta=viewport[i%2]-[1280,1024][i%2];
                r[i]+=match n(self.node,key,0) {0=>delta/2,2=>delta,_=>0};
            }
        }
        r
    }
    fn client_size(&self,viewport:[i32;2],offsets:&BTreeMap<String,[i32;4]>)->[i32;2] {
        let r=self.effective(viewport);let o=offsets.get(&self.node.id).copied().unwrap_or([0;4]);
        [r[2]-r[0]-o[0]-o[2],r[3]-r[1]-o[1]-o[3]]
    }
    fn apply_modes(&mut self,available:[i32;2]) {
        let (x,w)=axis(self.raw[0],self.raw[2]-self.raw[0],available[0],n(self.node,"LAYOUTMODEH",0));
        let (y,h)=axis(self.raw[1],self.raw[3]-self.raw[1],available[1],n(self.node,"LAYOUTMODEV",0));
        self.raw=[x,y,x+w,y+h];
    }
    fn relayout(&mut self,available:[i32;2],viewport:[i32;2],cards:&BTreeMap<String,usize>,offsets:&BTreeMap<String,[i32;4]>,scene:&Scenario) {
        // GuiWnd_RelayoutTree 00D6FCC0: modes -> pre-hook -> children -> post-hook -> modes.
        self.apply_modes(available);
        let client=self.client_size(viewport,offsets);
        let selected=self.node.children.get(*cards.get(&self.node.id).unwrap_or(&0)).map(|n|n.id.as_str());
        for child in &mut self.children {
            if !child.visible || (self.node.kind=="CARDLAYOUT"&&Some(child.node.id.as_str())!=selected) {continue}
            child.relayout(client,viewport,cards,offsets,scene);
        }
        if self.node.kind=="BOXLAYOUT" {
            // Assembly 00D8DC60: count advances before placement; behavior !=0 ignores count limit.
            let primary=if n(self.node,"LINE_ORDER",1)==0 {0} else {1};let cross=1-primary;
            let gaps=[n(self.node,"XSPACE",0),n(self.node,"YSPACE",0)];
            let direction=[n(self.node,"LEFT_TO_RIGHT",1)!=0,n(self.node,"TOP_TO_BOTTOM",1)!=0];
            let automatic=n(self.node,"BOXLAYOUT_BEHAVIOUR",0)!=0;
            let mut cursor=[0,0];let mut count=0;let mut maximum=0;
            for child in &mut self.children {
                if !child.visible {continue}
                let e=child.effective(viewport);let size=[e[2]-e[0],e[3]-e[1]];
                count+=1;
                let at=|cursor:[i32;2]| [if direction[0]{cursor[0]}else{client[0]-cursor[0]-size[0]},if direction[1]{cursor[1]}else{client[1]-cursor[1]-size[1]}];
                let mut position=at(cursor);
                let overflow=position[primary]<0 || position[primary]+size[primary]>client[primary];
                // X overflow is gated by count>1; Y overflow is not. This engine asymmetry is intentional.
                if automatic&&overflow&&(primary==1||count>1) {
                    maximum=maximum.max(cursor[cross]+size[cross]);
                    cursor[cross]=maximum+gaps[cross];cursor[primary]=0;maximum=0;
                    position=at(cursor);
                }
                child.raw=[position[0],position[1],position[0]+size[0],position[1]+size[1]];
                maximum=maximum.max(cursor[cross]+size[cross]);
                if automatic||count<n(self.node,"LINECOLUMNCOUNT",1) {cursor[primary]+=size[primary]+gaps[primary];}
                else {cursor[cross]=maximum+gaps[cross];cursor[primary]=0;maximum=0;count=0;}
            }
        }
        if let Some(handler)=self.node.handlers.iter().find(|h|h.event=="OnRender") {
            for action in &handler.actions {
                if action.key=="FITSIZETOCHILDREN" {self.fit(&action.value,viewport,cards,offsets);}
                if action.key=="UPDATE_INFOBAR_ISLAND_INFO"&&self.node.name=="INFOBAR_ISLAND_INFO"{if let Some(info)=&scene.island_info{
                    // 0048C2D0 runs at this exact point in the source action order.
                    // Full-viewport borderless parent makes local/absolute centers equal.
                    let mut width=self.client_size(viewport,offsets)[0];
                    if width<1{self.fit("HORIZONTAL",viewport,cards,offsets);width=self.client_size(viewport,offsets)[0];}
                    let x=(info.parent_viewport[0]/2-width/2).max(info.wares_right);
                    self.raw=[x,self.raw[1],x+width,self.raw[3]];
                }}
            }
        }
        self.apply_modes(available);
    }
    fn fit(&mut self,args:&str,viewport:[i32;2],cards:&BTreeMap<String,usize>,offsets:&BTreeMap<String,[i32;4]>) {
        let parts:Vec<_>=args.split(';').map(str::trim).collect();
        // 005FEB30 uses 00B08440's case-insensitive comparison for ASCII keywords.
        let mode=match parts.first().copied().unwrap_or("").to_ascii_uppercase().as_str(){"HORIZONTAL"=>0,"VERTICAL"=>1,"VERTICAL2"=>2,_=>3};
        let anchor=parts.get(1).is_some_and(|p|p.eq_ignore_ascii_case("RIGHT"));
        let selected=self.node.children.get(*cards.get(&self.node.id).unwrap_or(&0)).map(|c|c.id.as_str());
        let visible=|c:&Geometry|c.visible&&(self.node.kind!="CARDLAYOUT"||Some(c.node.id.as_str())==selected);
        if self.node.kind=="BOXLAYOUT" {
            let shift_x=(mode==0||mode==3)&&n(self.node,"LEFT_TO_RIGHT",1)==0;
            let shift_y=mode!=0&&n(self.node,"TOP_TO_BOTTOM",1)==0;
            let mut minimum=[10_000_000;2];
            for c in self.children.iter().filter(|c|visible(c)) {let e=c.effective(viewport);minimum[0]=minimum[0].min(e[0]);minimum[1]=minimum[1].min(e[1]);}
            let shift=[if shift_x{minimum[0]}else{0},if shift_y{minimum[1]}else{0}];
            for c in self.children.iter_mut().filter(|c|visible(c)) {let e=c.effective(viewport);c.raw=[e[0]-shift[0],e[1]-shift[1],e[2]-shift[0],e[3]-shift[1]];}
        }
        let mut far=[0,0];let mut near=[0,0];
        for c in self.children.iter().filter(|c|visible(c)) {far[0]=far[0].max(c.raw[2]);far[1]=far[1].max(c.raw[3]);near[0]=near[0].min(c.raw[0]);near[1]=near[1].min(c.raw[1]);}
        let mut size=[far[0]-near[0],far[1]-near[1]];
        for axis in 0..2 {
            let limit=|i:usize|parts.get(i).filter(|s|!s.is_empty()).map(|s|s.parse::<i32>().unwrap_or(0)).unwrap_or(-1);
            let min=limit(4+axis);let max=limit(2+axis);
            if min!=-1&&size[axis]<=min {size[axis]=min}if max>0&&size[axis]>max {size[axis]=max}
        }
        let client=self.client_size(viewport,offsets);
        if mode!=1&&mode!=2 {
            let border=if size[0]<1{0}else{self.raw[2]-self.raw[0]-client[0]};
            if anchor {self.raw[0]=self.raw[2]-border-size[0]}else{self.raw[2]=self.raw[0]+border+size[0]}
        }
        if mode!=0 {
            let border=if size[1]<1{0}else{self.raw[3]-self.raw[1]-client[1]};
            if mode==2 {self.raw[1]=self.raw[3]-border-size[1]}else{self.raw[3]=self.raw[1]+border+size[1]}
        }
    }
}
pub fn preview(root:&Widget,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,offsets:&BTreeMap<String,[i32;4]>)->Preview {
    preview_scenario(root,revision,width,height,show_hidden,cards,offsets,&BTreeMap::new())
}
pub fn preview_scenario(root:&Widget,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,offsets:&BTreeMap<String,[i32;4]>,visibility:&BTreeMap<String,bool>)->Preview {
    preview_text_scenario(root,revision,width,height,show_hidden,cards,offsets,visibility,&BTreeMap::new())
}
pub fn preview_text_scenario(root:&Widget,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,offsets:&BTreeMap<String,[i32;4]>,visibility:&BTreeMap<String,bool>,text_sizes:&BTreeMap<String,TextSize>)->Preview {
    preview_scene(root,revision,width,height,show_hidden,cards,offsets,visibility,text_sizes,&Scenario::default())
}
pub fn preview_scene(root:&Widget,revision:u64,width:i32,height:i32,show_hidden:bool,cards:&BTreeMap<String,usize>,offsets:&BTreeMap<String,[i32;4]>,visibility:&BTreeMap<String,bool>,text_sizes:&BTreeMap<String,TextSize>,scene:&Scenario)->Preview {
    let viewport=[width,height];let mut geometry=Geometry::new(root,visibility,offsets,text_sizes);
    fn raw_tree(g:&Geometry,out:&mut Vec<[i32;4]>){out.push(g.raw);for child in &g.children{raw_tree(child,out)}}
    let mut passes=vec![];
    for index in 1..=scene.passes{
        let mut before=vec![];raw_tree(&geometry,&mut before);
        geometry.relayout(viewport,viewport,cards,offsets,scene);
        let mut after=vec![];raw_tree(&geometry,&mut after);
        passes.push(Pass{index,root_raw:geometry.raw,identical_to_previous:before==after});
    }
    struct Builder<'a>{viewport:[i32;2],show_hidden:bool,cards:&'a BTreeMap<String,usize>,offsets:&'a BTreeMap<String,[i32;4]>,controller:bool,rects:Vec<Rect>}
    impl Builder<'_> {
        fn render(&mut self,g:&Geometry,parent:Option<&Rect>) {
            let node=g.node;
            if !self.show_hidden&&!g.visible {return}
            let e=g.effective(self.viewport);let o=self.offsets.get(&node.id).copied().unwrap_or([0;4]);
            let origin=parent.map(|p|[p.client[0],p.client[1]]).unwrap_or([0,0]);
            let (x,y)=(origin[0]+e[0],origin[1]+e[1]);let (w,h)=(e[2]-e[0],e[3]-e[1]);
            let viewport=[0,0,self.viewport[0],self.viewport[1]];
            let clip=if n(node,"CLIPPINGDISABLED",0)!=0 {viewport}else{parent.map(|p|intersect(p.clip,p.client)).unwrap_or(viewport)};
            let own_alpha=node.properties.iter().find(|p|p.key=="ALPHA").map(|p|if p.value.is_empty(){0.0}else{p.value.parse::<f32>().unwrap_or(1.0)}).unwrap_or(1.0);
            let alpha=parent.map(|p|own_alpha.min(p.alpha)).unwrap_or(own_alpha);
            let mut diagnostics=vec![];
            for p in &node.properties {
                if p.key.starts_with("ROTATION_SETTINGS/")||p.key=="BACKGROUNDCOLOR"||p.key=="BACKGROUND_TECHNIQUE" {diagnostics.push(format!("{} rendering effect not yet validated",p.key));}
            }
            for handler in &node.handlers {for action in &handler.actions {
                if handler.event=="OnRender"&&action.key=="FITSIZETOCHILDREN"{continue}
                if handler.event=="OnRender"&&action.key=="UPDATE_INFOBAR_ISLAND_INFO"&&self.controller&&node.name=="INFOBAR_ISLAND_INFO" {diagnostics.push("Island-info positioning used explicit controller inputs in this scenario; fertility/resource slot refresh still needs independent inputs".into());continue}
                diagnostics.push(format!("{}/{} requires runtime/scenario inputs; this verb is not executed",handler.event,action.key));
            }}
            for key in ["SCROLLBAR_HORZ","SCROLLBAR_VERT","SCALECENTER","RENDERTORT"] {if n(node,key,0)!=0{diagnostics.push(format!("{key} engine behavior not yet implemented"));}}
            if node.properties.iter().any(|p|p.key=="BACKGROUND"&&!p.value.is_empty())&&!self.offsets.contains_key(&node.id){diagnostics.push("Background client offsets unresolved for this state".into());}
            if self.show_hidden&&!g.visible {diagnostics.push("Hidden widget shown for editing; excluded from game layout participation".into());}
            let rect=Rect{id:node.id.clone(),parent:parent.map(|p|p.id.clone()),x,y,w,h,raw_relative:g.raw,effective_relative:e,clip,client:[x+o[0],y+o[1],x+w-o[2],y+h-o[3]],alpha,diagnostics};
            self.rects.push(rect.clone());
            let selected=node.children.get(*self.cards.get(&node.id).unwrap_or(&0)).map(|n|n.id.as_str());
            for child in &g.children {
                if node.kind=="CARDLAYOUT"&&Some(child.node.id.as_str())!=selected {continue}
                self.render(child,Some(&rect));
            }
        }
    }
    let mut b=Builder{viewport,show_hidden,cards,offsets,controller:scene.island_info.is_some(),rects:vec![]};b.render(&geometry,None);
    let mut diagnostics=vec!["Unit-scale pre/child/post layout with explicitly requested passes. Dynamic verbs, generated scrollbars, scaling and type-specific overrides remain incomplete.".into()];
    if scene.island_info.is_some(){diagnostics.push("Island-info positioning only: full-viewport parent and external wares edge are explicit inputs. Slot visibility/icon/count updates and show/sequence timing are not executed. Identical consecutive geometry does not prove native transition timing.".into());}
    Preview{revision,rects:b.rects,background_states:BTreeMap::new(),text_sizes:text_sizes.clone(),passes,diagnostics}
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Property;
    fn node(id: &str, kind: &str, props: &[(&str, &str)], children: Vec<Widget>) -> Widget {
        Widget {
            id: id.into(),
            kind: kind.into(),
            name: id.into(),
            properties: props
                .iter()
                .map(|(k, v)| Property {
                    key: k.to_string(),
                    value: v.to_string(),
                    source: "fixture".into(),
                    local: true,
                })
                .collect(),
            layers: vec![],
            warnings: vec![],
            children,
            handlers:vec![],
        }
    }
    #[test]fn fit_keywords_and_right_anchor_are_case_insensitive(){
        let child=node("child","LABEL",&[("EXTENT/WIDTH","20"),("EXTENT/HEIGHT","10")],vec![]);
        let mut root=node("root","LABEL",&[("EXTENT/LEFT","5"),("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","41")],vec![child]);
        root.handlers.push(crate::editor::Handler{event:"OnRender".into(),actions:vec![crate::editor::Property{key:"FITSIZETOCHILDREN".into(),value:"Horizontal;right".into(),source:"fixture".into(),local:true}],source:"fixture".into()});
        let p=preview(&root,1,1280,1024,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[0].raw_relative,[85,0,105,41]);
    }
    #[test]fn explicit_update_passes_preserve_geometry_and_run_controller_before_fit(){
        let child=node("child","LABEL",&[("EXTENT/WIDTH","232"),("EXTENT/HEIGHT","20")],vec![]);
        let mut root=node("root","FRAME",&[("PATHID","INFOBAR_ISLAND_INFO"),("EXTENT/LEFT","508"),("EXTENT/TOP","1"),("EXTENT/WIDTH","489"),("EXTENT/HEIGHT","228"),("ALIGNMENT_LEFTHORIZONTAL","1"),("ALIGNMENT_RIGHTHORIZONTAL","1"),("ALIGNMENT_TOPVERTICAL","1"),("ALIGNMENT_BOTTOMVERTICAL","1")],vec![child]);
        root.name="INFOBAR_ISLAND_INFO".into();
        root.handlers.push(crate::editor::Handler{event:"OnRender".into(),actions:vec![
            crate::editor::Property{key:"UPDATE_INFOBAR_ISLAND_INFO".into(),value:String::new(),source:"fixture".into(),local:true},
            crate::editor::Property{key:"FITSIZETOCHILDREN".into(),value:"HORIZONTAL".into(),source:"fixture".into(),local:true}],source:"fixture".into()});
        let scene=Scenario{passes:8,island_info:Some(IslandInfoScenario{parent_viewport:[1440,1080],wares_right:430})};
        let p=preview_scene(&root,1,1440,1080,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),&scene);
        assert_eq!(p.passes[0].root_raw,[476,1,708,229]);
        assert_eq!(p.passes[1].root_raw,[604,1,836,229]);
        assert!(p.passes[2].identical_to_previous);
        assert!(p.passes.iter().skip(1).all(|pass|pass.root_raw[0]==604),"{:?}",p.passes.iter().map(|pass|(pass.root_raw,pass.identical_to_previous)).collect::<Vec<_>>());
        let clamped=Scenario{passes:1,island_info:Some(IslandInfoScenario{parent_viewport:[1440,1080],wares_right:650})};
        let p=preview_scene(&root,1,1440,1080,false,&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),&BTreeMap::new(),&clamped);
        assert_eq!(p.passes[0].root_raw,[650,1,882,229]);
        assert!(Scenario{passes:33,island_info:None}.validate().is_err());
    }
    #[test]fn measured_text_grows_background_and_anchors_both_axes_before_box_fit(){
        let label=node("label","LABEL",&[("EXTENT/LEFT","11"),("EXTENT/TOP","9"),("EXTENT/WIDTH","96"),("EXTENT/HEIGHT","22")],vec![]);
        let offsets=BTreeMap::from([("label".into(),[2,3,4,5])]);
        let sizes=BTreeMap::from([("label".into(),TextSize{client:[90,22],far_anchor:false,pre_refresh_width:None})]);
        let p=preview_text_scenario(&label,1,1280,1024,false,&BTreeMap::new(),&offsets,&BTreeMap::new(),&sizes);assert_eq!(p.rects[0].raw_relative,[11,9,107,39]);assert_eq!(p.rects[0].client,[13,12,103,34]);
        let far=BTreeMap::from([("label".into(),TextSize{client:[20,10],far_anchor:true,pre_refresh_width:None})]);
        let p=preview_text_scenario(&label,1,1280,1024,false,&BTreeMap::new(),&offsets,&BTreeMap::new(),&far);assert_eq!(p.rects[0].raw_relative,[81,13,107,31]);
        let empty=BTreeMap::from([("label".into(),TextSize{client:[0,0],far_anchor:false,pre_refresh_width:None})]);
        let p=preview_text_scenario(&label,1,1280,1024,false,&BTreeMap::new(),&offsets,&BTreeMap::new(),&empty);assert_eq!([p.rects[0].w,p.rects[0].h],[7,9]);
        let sibling=node("sibling","LABEL",&[("EXTENT/WIDTH","10"),("EXTENT/HEIGHT","10")],vec![]);
        let mut root=node("root","BOXLAYOUT",&[("EXTENT/WIDTH","200"),("EXTENT/HEIGHT","100"),("LINE_ORDER","0"),("LINECOLUMNCOUNT","10")],vec![label,sibling]);
        root.handlers.push(crate::editor::Handler{event:"OnRender".into(),actions:vec![crate::editor::Property{key:"FITSIZETOCHILDREN".into(),value:"HORIZONTAL".into(),source:"fixture".into(),local:true}],source:"fixture".into()});
        let p=preview_text_scenario(&root,1,1280,1024,false,&BTreeMap::new(),&offsets,&BTreeMap::new(),&sizes);assert_eq!(p.rects[0].w,106);assert_eq!(p.rects[2].raw_relative[0],96);
    }
    #[test]
    fn client_fill_center_and_clip_escape() {
        let c = node(
            "child",
            "LABEL",
            &[
                ("EXTENT/WIDTH", "21"),
                ("EXTENT/HEIGHT", "20"),
                ("LAYOUTMODEH", "3"),
                ("CLIPPINGDISABLED", "1"),
            ],
            vec![],
        );
        let root = node(
            "root",
            "FRAME",
            &[
                ("EXTENT/LEFT", "10"),
                ("EXTENT/TOP", "20"),
                ("EXTENT/WIDTH", "100"),
                ("EXTENT/HEIGHT", "80"),
                ("ALIGNMENT_LEFTHORIZONTAL", "1"),
                ("ALIGNMENT_RIGHTHORIZONTAL", "1"),
                ("ALIGNMENT_TOPVERTICAL", "1"),
                ("ALIGNMENT_BOTTOMVERTICAL", "1"),
            ],
            vec![c],
        );
        let p = preview(
            &root,
            1,
            300,
            200,
            false,
            &BTreeMap::new(),
            &BTreeMap::from([("root".into(), [5, 7, 10, 12])]),
        );
        assert_eq!(p.rects[0].client, [15, 27, 100, 88]);
        assert_eq!(p.rects[1].x, 47);
        assert_eq!(p.rects[1].y, 27);
        assert_eq!(p.rects[1].clip, [0, 0, 300, 200]);
    }
    #[test]
    fn boxes_wrap_using_maximum_cross_size() {
        let children = vec![
            node(
                "a",
                "LABEL",
                &[("EXTENT/WIDTH", "10"), ("EXTENT/HEIGHT", "20")],
                vec![],
            ),
            node(
                "b",
                "LABEL",
                &[("EXTENT/WIDTH", "15"), ("EXTENT/HEIGHT", "30")],
                vec![],
            ),
            node(
                "c",
                "LABEL",
                &[("EXTENT/WIDTH", "20"), ("EXTENT/HEIGHT", "10")],
                vec![],
            ),
        ];
        let root = node(
            "r",
            "BOXLAYOUT",
            &[
                ("EXTENT/WIDTH", "100"),
                ("EXTENT/HEIGHT", "100"),
                ("LINE_ORDER", "0"),
                ("LINECOLUMNCOUNT", "2"),
                ("XSPACE", "4"),
                ("YSPACE", "3"),
            ],
            children,
        );
        let p = preview(
            &root,
            1,
            100,
            100,
            false,
            &BTreeMap::new(),
            &BTreeMap::new(),
        );
        assert_eq!((p.rects[1].x, p.rects[1].y), (0, 0));
        assert_eq!((p.rects[2].x, p.rects[2].y), (14, 0));
        assert_eq!((p.rects[3].x, p.rects[3].y), (0, 33));
    }

    #[test]
    fn frame_accessor_offsets_change_edges_without_rewriting_authored_extent() {
        let root=node("frame","FRAME",&[("EXTENT/LEFT","10"),("EXTENT/TOP","3"),("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","50"),("ALIGNMENT_RIGHTHORIZONTAL","2"),("ALIGNMENT_TOPVERTICAL","1"),("ALIGNMENT_BOTTOMVERTICAL","1")],vec![]);
        let p=preview(&root,1,1440,1080,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[0].raw_relative,[10,3,110,53]);
        assert_eq!(p.rects[0].effective_relative,[90,3,270,53]);
        assert_eq!((p.rects[0].x,p.rects[0].w),(90,180));
    }

    #[test]
    fn post_layout_box_uses_resolved_child_size_and_overwrites_near_mode_position() {
        let child=node("c","LABEL",&[("EXTENT/WIDTH","7"),("EXTENT/HEIGHT","10"),("LAYOUTMODEH","1"),("LAYOUTMODEV","2")],vec![]);
        let root=node("r","BOXLAYOUT",&[("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","60"),("LINE_ORDER","0"),("LINECOLUMNCOUNT","1"),("YSPACE","5")],vec![child.clone(),node("d","LABEL",&[("EXTENT/WIDTH","8"),("EXTENT/HEIGHT","9"),("LAYOUTMODEV","2")],vec![])]);
        let p=preview(&root,1,100,60,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[1].raw_relative,[0,0,100,10]);
        assert_eq!(p.rects[2].raw_relative,[0,15,8,24]);
    }

    #[test]
    fn automatic_wrap_ignores_count_and_accounts_for_current_candidate_cross_size() {
        let children=vec![node("a","LABEL",&[("EXTENT/WIDTH","60"),("EXTENT/HEIGHT","10")],vec![]),node("b","LABEL",&[("EXTENT/WIDTH","60"),("EXTENT/HEIGHT","30")],vec![])];
        let root=node("r","BOXLAYOUT",&[("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","100"),("LINE_ORDER","0"),("LINECOLUMNCOUNT","1"),("BOXLAYOUT_BEHAVIOUR","1"),("YSPACE","3")],children);
        let p=preview(&root,1,100,100,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[1].raw_relative,[0,0,60,10]);
        assert_eq!(p.rects[2].raw_relative,[0,33,60,63]);
    }

    #[test]
    fn reverse_automatic_wrap_and_first_oversized_child_match_engine_asymmetry() {
        let child=node("c","LABEL",&[("EXTENT/WIDTH","120"),("EXTENT/HEIGHT","120")],vec![]);
        let root=node("r","BOXLAYOUT",&[("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","100"),("LINE_ORDER","0"),("BOXLAYOUT_BEHAVIOUR","1"),("LEFT_TO_RIGHT","0")],vec![child]);
        let p=preview(&root,1,100,100,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[1].raw_relative,[-20,0,100,120]);
        let mut root=root;root.properties.iter_mut().find(|p|p.key=="LINE_ORDER").unwrap().value="1".into();
        let p=preview(&root,1,100,100,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[1].raw_relative,[-140,0,-20,120]);
    }

    #[test]
    fn effective_alpha_is_minimum_and_hidden_inspection_does_not_change_box_flow() {
        let child=node("c","LABEL",&[("ALPHA","0.6"),("EXTENT/HEIGHT","10"),("HIDDEN","1")],vec![]);
        let root=node("r","BOXLAYOUT",&[("ALPHA","0.5"),("LINE_ORDER","0"),("EXTENT/WIDTH","100"),("EXTENT/HEIGHT","100")],vec![child,node("d","LABEL",&[("EXTENT/HEIGHT","10")],vec![])]);
        let p=preview(&root,1,100,100,true,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[1].alpha,0.5);
        assert_eq!(p.rects[2].y,0);
    }
    #[test]
    fn fit_union_negative_coordinates_limits_and_client_border() {
        let mut root=node("r","LABEL",&[("EXTENT/LEFT","50"),("EXTENT/TOP","60"),("EXTENT/WIDTH","200"),("EXTENT/HEIGHT","100")],vec![node("c","LABEL",&[("EXTENT/LEFT","-10"),("EXTENT/TOP","-5"),("EXTENT/WIDTH","40"),("EXTENT/HEIGHT","20")],vec![])]);
        root.handlers.push(crate::editor::Handler{event:"OnRender".into(),source:"fixture".into(),actions:vec![Property{key:"FITSIZETOCHILDREN".into(),value:"; RIGHT; 35; 50; 30; 25".into(),source:"fixture".into(),local:true}]});
        let p=preview(&root,1,500,500,false,&BTreeMap::new(),&BTreeMap::from([("r".into(),[2,3,4,5])]));
        assert_eq!(p.rects[0].raw_relative,[209,60,250,93]);
        root.handlers[0].actions[0].value="VERTICAL2".into();
        let p=preview(&root,1,500,500,false,&BTreeMap::new(),&BTreeMap::new());
        assert_eq!(p.rects[0].raw_relative,[50,140,250,160]);
    }
}
