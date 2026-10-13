//! Recovered local text-part layout. This does not rasterize glyphs or choose an owner origin.
use crate::native_text::{self,FontSpec};
use serde::{Deserialize,Serialize};
#[derive(Clone,Deserialize)]
pub struct Font {
    #[serde(flatten)] pub spec:FontSpec,
    #[serde(rename="DefaultColor")] pub default_color:Option<u32>,
}
#[derive(Debug,Clone,Serialize,PartialEq)]
pub struct Part {
    pub text:String,pub size:[i32;2],pub offset:[i32;2],pub line_rect:[i32;4],
    pub font:Option<String>,pub color:u32,pub icon_guid:Option<i32>,pub icon_variation:i32,pub icon_clip:bool,
}
#[derive(Debug,PartialEq)] enum Token {Plain(String),Tag(String),Prefix(String)}
#[cfg(test)]fn tokenize(text:&str)->Result<Vec<Token>,String>{
    tokenize_scenario(text,None)
}
fn tokenize_scenario(text:&str,current_city_name:Option<&str>)->Result<Vec<Token>,String>{
    if text.encode_utf16().count()>16384||text.chars().any(|c|c as u32>0xffff||c.is_control()) {return Err("Control text, surrogate code units or source over16K need separate handling".into())}
    if current_city_name.is_some_and(|v|v.encode_utf16().count()>512||v.chars().any(|c|c as u32>0xffff||c.is_control())){return Err("City-name scenario needs at most512 BMP noncontrol code units".into())}
    let mut units:Vec<char>=text.chars().collect();let mut pos=0;let mut out=vec![];let mut insertions=0;
    while pos<units.len(){
        if units[pos]=='[' {
            pos+=1;let mut tag=String::new();let start=pos;
            loop {if pos>=units.len(){return Err("Unterminated inline tag".into())}if pos-start>=2047{return Err("Tag exceeds native scan limit".into())}let c=units[pos];pos+=1;
                if c=='\\'{let escaped=*units.get(pos).ok_or("Trailing escape")?;pos+=1;tag.push(escaped)}
                else if c==']'||c==';'{
                    // DAF170 rewrites ';' as '[' and rescans it. DAF5C0 inserts
                    // callback UTF16 at that active cursor; retain pending plain
                    // prefixes and scan the inserted value with the source suffix.
                    if c==';'{pos-=1;units[pos]='[';}
                    if tag.split(' ').next()==Some("CURRENT_CITY_NAME"){
                        let value=current_city_name.ok_or("Missing CURRENT_CITY_NAME scenario value")?;insertions+=1;
                        if insertions>64||units.len()+value.chars().count()>16384{return Err("Runtime tag insertion bound exceeded".into())}
                        units.splice(pos..pos,value.chars());
                    }else{out.push(Token::Tag(tag));}break
                }else{tag.push(c)}
            }
        }else{
            let mut token=String::new();let start=pos;
            loop {if pos>=units.len(){out.push(Token::Plain(token));break}if pos-start>=2047{return Err("Plain token exceeds native scan limit".into())}
                let c=units[pos];if c=='['{out.push(Token::Prefix(token));break}pos+=1;
                if c=='\\'{let escaped=*units.get(pos).ok_or("Trailing escape")?;pos+=1;token.push(escaped)}else{token.push(c)}
                if token.ends_with(' '){out.push(Token::Plain(token));break}
            }
        }
    }Ok(out)
}
pub struct Input<'a> {pub text:&'a str,pub font:&'a str,pub fonts:&'a [Font],pub scale:[f32;2],pub flags:u32,pub width:i32,pub color:Option<u32>}
#[derive(Debug,Serialize)]pub struct ResultLayout {pub parts:Vec<Part>,pub size:[i32;2]}
struct Builder<'a> {input:Input<'a>,font:usize,color:u32,align:u8,pending:String,line:Vec<Part>,parts:Vec<Part>,x:i32,y:i32,cursor_bottom:i32,right:i32,bottom:i32,size:[i32;2]}
impl<'a> Builder<'a> {
    fn font_index(&self,name:&str)->Result<usize,String>{let mut candidates=self.input.fonts.iter().enumerate().filter(|(_,f)|f.spec.name==name);let index=candidates.next().map(|(i,_)|i).ok_or_else(||format!("Font missing: {name}"))?;if candidates.next().is_some(){return Err(format!("Font ambiguous: {name}"))}Ok(index)}
    fn metrics(&self,text:&str)->Result<[i32;2],String>{Ok(native_text::measure(&self.input.fonts[self.font].spec,text,self.input.scale)?.size)}
    fn default_color(&self)->Result<u32,String>{if self.input.flags&4!=0 {return self.input.color.ok_or("Explicit layout color not captured".into())}let font=&self.input.fonts[self.font];if font.spec.flags&0x80!=0{font.default_color.ok_or("Font default color missing".into())}else{Ok(u32::MAX)}}
    fn finish(&mut self){
        // DC69B0 accumulates the far edges of appended lines independently of
        // their glyph parts. DB0670 appends empty preceding CR lines too.
        self.size[0]=self.size[0].max(self.right);self.size[1]=self.size[1].max(self.bottom);
        if self.input.flags&0x40!=0&&!self.line.is_empty(){
            let first=self.line.first().unwrap();let last=self.line.last().unwrap();let gap=first.offset[0]+self.input.width-last.size[0]-last.offset[0];
            if gap>0 {let shift=if self.align==2{gap/2}else if self.align==1{gap}else{0};for part in &mut self.line {part.offset[0]+=shift;}}
        }
        for part in &mut self.line {part.line_rect=[0,self.y,self.right,self.bottom];}
        self.parts.append(&mut self.line);
    }
    // DB0670 and DB0140 use the current part advance, not the line's union bounds.
    // Empty CR lines retain font-height bounds but do not advance this cursor.
    fn new_line(&mut self,min_height:i32){self.x=0;self.y=self.cursor_bottom;self.right=0;self.bottom=self.y+min_height;}
    fn append(&mut self,mut part:Part)->Result<(),String>{
        let far=self.x.checked_add(part.size[0]).ok_or("Text width overflow")?;
        if self.input.flags&0x40!=0&&far>self.input.width {
            if self.input.flags&0x4000!=0{return Err("Ellipsis layout not implemented".into())}
            if self.input.flags&0x1000==0 {if let Some(last)=self.line.last_mut(){if last.font.is_some()&&last.text.ends_with(' '){last.text.pop();let name=last.font.clone().unwrap();let index=self.font_index(&name)?;let size=native_text::measure(&self.input.fonts[index].spec,&self.line.last().unwrap().text,self.input.scale)?.size;self.line.last_mut().unwrap().size=size;}}}
            if self.input.flags&0x80!=0&&self.x>0{self.finish();self.new_line(0);}
            if self.input.flags&0x80!=0&&part.size[0]>self.input.width {return Err("Oversized-word splitting not implemented".into())}
            // With no wrap or no possible prefix, the game keeps an overflowing part.
        }
        part.offset=[self.x,self.y];self.x=self.x.checked_add(part.size[0]).ok_or("Text width overflow")?;
        self.cursor_bottom=self.y+part.size[1];self.right=self.right.max(self.x);self.bottom=self.bottom.max(self.cursor_bottom);self.line.push(part);Ok(())
    }
    fn plain(&mut self,text:String)->Result<(),String>{if text.is_empty(){return Ok(())}let size=self.metrics(&text)?;self.append(Part{text,size,offset:[0;2],line_rect:[0;4],font:Some(self.input.fonts[self.font].spec.name.clone()),color:self.color,icon_guid:None,icon_variation:0,icon_clip:false})}
    fn flush(&mut self)->Result<(),String>{let pending=std::mem::take(&mut self.pending);self.plain(pending)}
    fn tag(&mut self,tag:&str)->Result<(),String>{
        if !tag.is_ascii(){return Err("Narrow tag encoding not implemented".into())}
        if let Some(name)=tag.strip_prefix("FONT "){self.font=self.font_index(name)?;self.flush()?;self.color=self.default_color()?;return Ok(())}
        if tag=="DEFAULTCOLOR"{self.flush()?;self.color=self.default_color()?;return Ok(())}
        if let Some(hex)=tag.strip_prefix("COLOR "){let color=u32::from_str_radix(hex.trim().trim_start_matches("0x"),16).map_err(|_|"Invalid COLOR")?;self.flush()?;self.color=color;return Ok(())}
        match tag {
            "CENTER"=>{self.align=2;return Ok(())},"RIGHTALIGN"=>{self.align=1;return Ok(())},"LEFTALIGN"=>{self.align=0;return Ok(())},
            "CR"=>{self.flush()?;self.finish();let height=self.metrics("x")?[1];self.new_line(height);return Ok(())},_=>{}
        }
        if tag.starts_with("CR "){return Err("Numeric CR uses cursor offsets and needs a separate fixture".into())}
        let (args,fixed)=if let Some(args)=tag.strip_prefix("AUTOSIZE_ICONGUID "){(args,false)}else if let Some(args)=tag.strip_prefix("FIXEDSIZE_ICONGUID "){(args,true)}else{return Err(format!("Unsupported inline tag [{tag}]"))};
        let numbers:Vec<_>=args.split_whitespace().map(|s|s.parse::<i32>().map_err(|_|"Invalid inline icon integer")).collect::<Result<_,_>>()?;
        let max=if fixed{4}else{3};if numbers.is_empty()||numbers.len()>max{return Err("Inline icon argument count".into())}
        let guid=numbers[0];if guid<0{return Err("Inline icon needs a resolved positive GUID".into())}let variation=*numbers.get(1).unwrap_or(&0);
        let side=if fixed{*numbers.get(2).unwrap_or(&16)}else{self.metrics("W")?[1]};if side<0||side>8192{return Err("Inline icon size outside preview bound".into())}
        let color=numbers.get(if fixed{3}else{2}).copied().unwrap_or(-1) as u32;
        self.append(Part{text:String::new(),size:[side,side],offset:[0;2],line_rect:[0;4],font:None,color,icon_guid:Some(guid),icon_variation:variation,icon_clip:self.input.flags&0x2000!=0})
    }
}
pub fn layout(input:Input)->Result<Vec<Part>,String>{
    layout_scenario(input,None)
}
pub fn layout_scenario(input:Input,current_city_name:Option<&str>)->Result<Vec<Part>,String>{
    Ok(layout_bounds_scenario(input,current_city_name)?.parts)
}
pub fn layout_bounds_scenario(input:Input,current_city_name:Option<&str>)->Result<ResultLayout,String>{
    if input.flags&!(6|0x40|0x80|0x100|0x800|0x1000|0x2000)!=0{return Err("Unimplemented text layout flags".into())}
    let tokens=tokenize_scenario(input.text,current_city_name)?;let align=if input.flags&0x100!=0{2}else if input.flags&0x800!=0{1}else{0};
    let mut builder=Builder{input,font:0,color:u32::MAX,align,pending:String::new(),line:vec![],parts:vec![],x:0,y:0,cursor_bottom:0,right:0,bottom:0,size:[0;2]};
    builder.font=builder.font_index(builder.input.font)?;builder.color=builder.default_color()?;
    for token in tokens {match token {Token::Plain(text)=>{let mut full=std::mem::take(&mut builder.pending);full.push_str(&text);builder.plain(full)?},Token::Prefix(text)=>builder.pending.push_str(&text),Token::Tag(tag)=>builder.tag(&tag)?}}
    builder.flush()?;
    // Final drain skips zero-width lines (DC2C50), including a trailing CR.
    if builder.right!=0{builder.finish();}
    Ok(ResultLayout{parts:builder.parts,size:builder.size})
}
#[cfg(test)] mod tests {use super::*;
    #[cfg(windows)]#[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn processed_bounds_follow_appended_lines_and_skip_final_empty_cr(){
        let repo=crate::editor::discover_repo().unwrap();let xml=crate::localisation::inspect(&repo,"eng",-1,"Headlines").unwrap().fonts[0].value.clone();let fonts=[Font{spec:crate::native_text::font_spec(&xml).unwrap(),default_color:None}];
        let run=|text|layout_bounds_scenario(Input{text,font:"Headlines",fonts:&fonts,scale:[1.0,1.0],flags:6,width:i32::MAX,color:Some(u32::MAX)},None).unwrap();
        assert_eq!(run("First Island").size,[90,22]);assert_eq!(run("First Island[CR]").size,[90,22]);
        let blank=run("[CR][CR]");assert!(blank.parts.is_empty());assert_eq!(blank.size,[0,22]);
        assert_eq!(run("First Island[CR][CR]").size,[90,44]);
        assert_eq!(run("").size,[0,0]);
    }
    #[test]fn callback_insertion_rescans_prefix_suffix_markup_and_semicolon(){
        assert_eq!(tokenize_scenario("pre[CURRENT_CITY_NAME]post",Some("First Island")).unwrap(),vec![Token::Prefix("pre".into()),Token::Plain("First ".into()),Token::Plain("Islandpost".into())]);
        assert_eq!(tokenize_scenario("[CURRENT_CITY_NAME;COLOR ffffffff]x",Some("[FONT Tooltip]City")).unwrap(),vec![Token::Tag("FONT Tooltip".into()),Token::Prefix("City".into()),Token::Tag("COLOR ffffffff".into()),Token::Plain("x".into())]);
        assert_eq!(tokenize_scenario("\\[CURRENT_CITY_NAME]",None).unwrap(),vec![Token::Plain("[CURRENT_CITY_NAME]".into())]);
        assert_eq!(tokenize_scenario("a[CURRENT_CITY_NAME ignored]b",Some("")).unwrap(),vec![Token::Prefix("a".into()),Token::Plain("b".into())]);
        assert!(tokenize_scenario("[CURRENT_CITY_NAME]",None).unwrap_err().contains("Missing"));
        assert!(tokenize_scenario("[CURRENT_CITY_NAME]",Some("[CURRENT_CITY_NAME]")).unwrap_err().contains("bound"));
    }
    #[test]fn token_prefix_tags_escapes_and_semicolon_rewrite(){assert_eq!(tokenize("a[COLOR ffffffff;FONT Default]b \\[x").unwrap(),vec![Token::Prefix("a".into()),Token::Tag("COLOR ffffffff".into()),Token::Tag("FONT Default".into()),Token::Plain("b ".into()),Token::Plain("[x".into())]);assert!(tokenize("[FONT X").is_err());}
}
