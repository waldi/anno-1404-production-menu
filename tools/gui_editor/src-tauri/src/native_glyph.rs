//! Own-process replica of the GUI atlas builder (Addon.exe A470E0).
//! Metadata agreement is a separate gate from shader/screenshot pixel parity.
use serde::{Deserialize,Serialize};
#[derive(Clone,Debug,Deserialize,Serialize)]
#[serde(rename_all="PascalCase")]
pub struct Font {
    pub name:String,pub face:String,pub width:i32,pub height:i32,pub weight:i32,
    pub italic:i32,pub quality:i32,pub outline:i32,pub underline:i32,
    #[serde(default)]pub outline_color:u32,#[serde(default)]pub shadow:i32,
    #[serde(default)]pub shadow_x:i32,#[serde(default)]pub shadow_y:i32,#[serde(default)]pub shadow_color:u32,
}
pub fn font_from_xml(xml:&str,scale:[f32;2])->Result<Font,String>{
    font_from_xml_context(xml,scale,None)
}
fn font_from_xml_context(xml:&str,scale:[f32;2],prior_shadow_color:Option<u32>)->Result<Font,String>{
    if scale.iter().any(|s|!s.is_finite()||*s<=0.0||*s>8.0){return Err("Invalid font scale".into())}
    let spec=crate::native_text::font_spec(xml)?;
    let doc=roxmltree::Document::parse(xml).map_err(|e|e.to_string())?;
    let mut outline_color=0;let mut shadow_color=prior_shadow_color;let mut shadow_offset=[0,0];
    for node in doc.root_element().children().filter(|n|n.is_element()){
        let value=node.text().unwrap_or("").trim();
        let color=||value.parse::<u32>().map_err(|_|format!("Unsupported decimal font color {}",node.tag_name().name()));
        match node.tag_name().name(){
            "Flag" if value=="Outline"=>outline_color=0xff000000,
            "OutlineBorderColor"=>outline_color=color()?,
            "ShadowColor"=>shadow_color=Some(color()?),
            "ShadowOffset"=>for(index,axis)in["x","y"].iter().enumerate(){if let Some(child)=node.children().find(|n|n.has_tag_name(*axis)){shadow_offset[index]=child.text().unwrap_or("").trim().parse::<i32>().map_err(|_|format!("Invalid shadow offset {axis}"))?;}},
            _=>{}
        }
    }
    let shadow=((spec.flags>>6)&1)as i32;
    let shadow_color=if shadow!=0{shadow_color.ok_or("Active shadow lacks explicit ShadowColor; constructor color is unverified")?}else{shadow_color.unwrap_or(0)};
    // EnsureCreated supplies zero LOGFONT width; XML relative-width metadata
    // does not become the GUI command's width in this recovered path.
    Ok(Font{name:spec.name,face:spec.face,width:0,height:if spec.flags&1!=0{(spec.height as f32*scale[1]+0.5)as i32}else{spec.height},weight:if spec.flags&0x10!=0{spec.weight}else{0},italic:((spec.flags>>1)&1)as i32,quality:if spec.flags&0x200!=0{5}else{3},outline:((spec.flags>>8)&1)as i32,underline:((spec.flags>>2)&1)as i32,outline_color,shadow,shadow_x:shadow_offset[0],shadow_y:shadow_offset[1],shadow_color})
}
pub fn font_from_document(xml:&str,name:&str,scale:[f32;2])->Result<Font,String>{
    let doc=roxmltree::Document::parse(xml).map_err(|e|e.to_string())?;
    if !doc.root_element().has_tag_name("Localisation"){return Err("Expected localization document".into())}
    let nodes:Vec<_>=doc.root_element().children().filter(|n|n.has_tag_name("Font")).collect();
    let target=|node:roxmltree::Node|node.children().find(|n|n.has_tag_name("Name")).and_then(|n|n.text()).map(str::trim)==Some(name);
    if nodes.iter().filter(|n|target(**n)).count()!=1{return Err("Named font missing or duplicated in selected document".into())}
    let mut shadow_color=None;
    for node in nodes{
        if target(node){return font_from_xml_context(&xml[node.range()],scale,shadow_color)}
        for field in node.children().filter(|n|n.has_tag_name("ShadowColor")){
            // Invalid earlier assignment makes the retained value unknown; a later
            // valid assignment can establish it again. No initial stack seed.
            shadow_color=field.text().and_then(|v|v.trim().parse::<u32>().ok());
        }
    }Err("Font not found".into())
}
#[derive(Debug,Clone,Deserialize,Serialize,PartialEq)]
#[serde(rename_all="PascalCase")]
pub struct Glyph {pub code:i32,pub width:i32,pub height:i32,pub advance:i32,pub bearing:i32,#[serde(rename="UV")]pub uv:[f32;4]}
#[derive(Serialize)]
pub struct Atlas {pub size:[u32;2],pub cached_height:i32,pub glyphs:Vec<Glyph>,pub alpha_png:String,pub outline_png:Option<String>,pub selected_face:String,pub scope:String,#[serde(skip)]pub alpha:Vec<u8>,#[serde(skip)]pub outline:Option<Vec<u8>>}
fn png_alpha(width:u32,height:u32,alpha:&[u8])->Result<String,String>{
    use base64::Engine;let mut bytes=vec![];{let mut encoder=png::Encoder::new(&mut bytes,width,height);encoder.set_color(png::ColorType::Grayscale);encoder.set_depth(png::BitDepth::Eight);let mut writer=encoder.write_header().map_err(|e|e.to_string())?;writer.write_image_data(alpha).map_err(|e|e.to_string())?;}
    Ok(format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)))
}
#[cfg(windows)]
pub fn build(font:&Font)->Result<Atlas,String>{
    build_page(font,0)
}
#[cfg(windows)]
pub fn build_page(font:&Font,page:u8)->Result<Atlas,String>{
    use windows_sys::Win32::{Foundation::RECT,Graphics::Gdi::*};use std::ptr::null_mut;
    if page==255{return Err("Native pageFF is rejected by the renderer".into())}
    if font.face.is_empty()||font.face.contains('\0')||font.face.encode_utf16().count()>31||!(-1024..=1024).contains(&font.height)||font.width.unsigned_abs()>1024||!(0..=1000).contains(&font.weight)||!(0..=6).contains(&font.quality){return Err("Invalid atlas font parameters".into())}
    struct Resources {dc:HDC,font:HFONT,old_font:HGDIOBJ,bitmap:HBITMAP,old_bitmap:HGDIOBJ}
    impl Drop for Resources {fn drop(&mut self){unsafe{if !self.old_bitmap.is_null(){SelectObject(self.dc,self.old_bitmap);}if !self.bitmap.is_null(){DeleteObject(self.bitmap);}if !self.old_font.is_null(){SelectObject(self.dc,self.old_font);}if !self.font.is_null(){DeleteObject(self.font);}if !self.dc.is_null(){DeleteDC(self.dc);}}}}
    unsafe {
        let mut r=Resources{dc:CreateCompatibleDC(null_mut()),font:null_mut(),old_font:null_mut(),bitmap:null_mut(),old_bitmap:null_mut()};if r.dc.is_null(){return Err("CreateCompatibleDC failed".into())}
        let mut lf:LOGFONTW=std::mem::zeroed();lf.lfHeight=font.height;lf.lfWidth=font.width;lf.lfWeight=font.weight;lf.lfItalic=(font.italic!=0)as u8;lf.lfUnderline=(font.underline!=0)as u8;lf.lfOutPrecision=4;lf.lfQuality=if font.outline!=0{3}else{font.quality as u8};for(i,c)in font.face.encode_utf16().enumerate(){lf.lfFaceName[i]=c;}
        r.font=CreateFontIndirectW(&lf);if r.font.is_null(){return Err("CreateFontIndirectW failed".into())}r.old_font=SelectObject(r.dc,r.font);if r.old_font.is_null()||r.old_font as isize == -1{r.old_font=null_mut();return Err("SelectObject font failed".into())}
        let mut tm:TEXTMETRICA=std::mem::zeroed();if GetTextMetricsA(r.dc,&mut tm)==0{return Err("GetTextMetricsA failed".into())}
        let mut face=[0u16;128];if GetTextFaceW(r.dc,128,face.as_mut_ptr())<=0{return Err("GetTextFaceW failed".into())}let selected_face=String::from_utf16_lossy(&face[..face.iter().position(|c|*c==0).unwrap_or(128)]);
        let extra=if font.outline!=0{2}else{0};let cached_height=tm.tmHeight+extra;
        let requested=[(tm.tmMaxCharWidth+extra).checked_mul(16).ok_or("Atlas width overflow")?,cached_height.checked_mul(16).ok_or("Atlas height overflow")?];
        if requested.iter().any(|n|*n<=2||*n>2048){return Err("Atlas outside recovered allocation bounds".into())}let width=(requested[0]as u32).next_power_of_two();let height=(requested[1]as u32).next_power_of_two();let cell=[width as i32/16,height as i32/16];
        let mut info:BITMAPINFO=std::mem::zeroed();info.bmiHeader.biSize=40;info.bmiHeader.biWidth=width as i32;info.bmiHeader.biHeight=-(height as i32);info.bmiHeader.biPlanes=1;info.bmiHeader.biBitCount=24;
        let mut bits=null_mut();r.bitmap=CreateDIBSection(r.dc,&info,DIB_RGB_COLORS,&mut bits,null_mut(),0);if r.bitmap.is_null()||bits.is_null(){return Err("CreateDIBSection failed".into())}r.old_bitmap=SelectObject(r.dc,r.bitmap);if r.old_bitmap.is_null()||r.old_bitmap as isize == -1{r.old_bitmap=null_mut();return Err("SelectObject bitmap failed".into())}
        let byte_len=width as usize*height as usize*3;std::ptr::write_bytes(bits.cast::<u8>(),0,byte_len);SetBkMode(r.dc,TRANSPARENT as i32);SetTextColor(r.dc,0xff0000);SetBkColor(r.dc,0);
        let mut glyphs=vec![];for index in 0u32..256 {
            let code=page as u32*256+index;
            let x=(index as i32%16)*cell[0];let y=(index as i32/16)*cell[1];let mut rect=RECT{left:x,top:y,right:x+cell[0],bottom:y+cell[1]};let mut abc:ABC=std::mem::zeroed();let mut bearing=0;
            let mut glyph_width=if GetCharABCWidthsW(r.dc,code,code,&mut abc)==0{let mut w=0;if GetCharWidth32A(r.dc,code,code,&mut w)==0{return Err(format!("GetCharWidth32A failed for {code}"))}w}else if abc.abcA>=0{abc.abcA+abc.abcB as i32+abc.abcC}else{rect.left-=abc.abcA;bearing=abc.abcA;abc.abcB as i32+abc.abcC};
            if font.outline!=0{rect.left+=1;rect.top+=1;}let unit=code as u16;let glyph_height=DrawTextW(r.dc,&unit,1,&mut rect,0x920)+extra;glyph_width+=extra;
            glyphs.push(Glyph{code:code as i32,width:glyph_width,height:glyph_height,advance:glyph_width,bearing,uv:[x as f32/width as f32,(x+glyph_width)as f32/width as f32,y as f32/height as f32,(y+glyph_height)as f32/height as f32]});
        }
        GdiFlush();let dib=std::slice::from_raw_parts_mut(bits.cast::<u8>(),byte_len);let alpha:Vec<_>=dib.chunks_exact(3).map(|p|p[0]).collect();let mut outline_data=None;let outline_png=if font.outline!=0{
            // Preserve A470E0's traversal and channel separation. Outline channel is
            // cleared on glyph coverage, then propagated to empty eight-neighbors.
            for x in 0..width as i32{for y in 0..height as i32{let i=(y as usize*width as usize+x as usize)*3;if dib[i]!=0{dib[i+1]=0;for dx in -1..=1{for dy in -1..=1{let(nx,ny)=(x+dx,y+dy);if nx>=0&&ny>=0&&nx<width as i32&&ny<height as i32{let n=(ny as usize*width as usize+nx as usize)*3;if dib[n]==0{dib[n+1]=255;}}}}}}}
            if font.quality!=3{for x in 0..width as i32{for y in 0..height as i32{let i=(y as usize*width as usize+x as usize)*3;if dib[i]==0&&dib[i+1]==0{let mut neighbors=0u8;for(dx,dy)in[(-1,0),(1,0),(0,-1),(0,1)]{let(nx,ny)=(x+dx,y+dy);if nx>=0&&ny>=0&&nx<width as i32&&ny<height as i32&&dib[(ny as usize*width as usize+nx as usize)*3+1]!=0{neighbors+=1;}}if neighbors>1{dib[i+2]=neighbors.wrapping_shl(6);}}}}}
            let outline:Vec<_>=dib.chunks_exact(3).map(|p|if p[1]!=0{p[1]}else{p[2]}).collect();let png=png_alpha(width,height,&outline)?;outline_data=Some(outline);Some(png)
        }else{None};
        Ok(Atlas{size:[width,height],cached_height,glyphs,alpha_png:png_alpha(width,height,&alpha)?,outline_png,selected_face,scope:"Own-process GDI atlas from recovered builder. Glyph metadata and raster masks; actual shader filtering/blending and screenshot pixels require separate verification.".into(),alpha,outline:outline_data})
    }
}
#[cfg(not(windows))]
pub fn build(_: &Font)->Result<Atlas,String>{Err("Native glyph atlas requires Windows GDI".into())}
#[cfg(not(windows))]
pub fn build_page(_: &Font,_:u8)->Result<Atlas,String>{Err("Native glyph atlas requires Windows GDI".into())}
#[derive(Deserialize)]pub struct RasterInput {pub font:Font,pub text:String,pub color:u32}
#[derive(Serialize)]pub struct Raster {pub size:[u32;2],pub origin:[i32;2],pub advance:i32,pub png:String,pub scope:String}
pub fn render_run(input:&RasterInput)->Result<Raster,String>{
    use base64::Engine;
    if input.text.is_empty()||input.text.encode_utf16().count()>512||input.text.chars().any(|c|c as u32>=0xff00||c.is_control()){return Err("Raster probe supports 1..512 non-control BMP units through pageFE; pageFF/non-BMP are unsupported".into())}
    if input.font.shadow_x.unsigned_abs()>2048||input.font.shadow_y.unsigned_abs()>2048{return Err("Shadow offset outside probe bounds".into())}
    let page_ids:std::collections::BTreeSet<_>=input.text.chars().map(|c|((c as u32)>>8)as u8).collect();if page_ids.len()>8{return Err("Sample exceeds eight glyph pages".into())}
    let mut pages=std::collections::BTreeMap::new();for page in page_ids{pages.insert(page,build_page(&input.font,page)?);}
    let mut pen=0;let mut left=0;let mut right=0;let mut top=0;let mut bottom=0;let mut quads=vec![];
    for c in input.text.chars(){let page=((c as u32)>>8)as u8;let atlas=&pages[&page];let g=&atlas.glyphs[(c as usize)&255];let x=pen+g.bearing;
        if input.font.shadow!=0{quads.push((x+input.font.shadow_x,input.font.shadow_y,g,(input.font.shadow_color&0xffffff)|(input.color&0xff000000),page));}quads.push((x,0,g,input.color,page));pen+=g.advance;
    }
    for(x,y,g,_,_)in &quads{left=left.min(*x);right=right.max(x+g.width);top=top.min(*y);bottom=bottom.max(y+g.height);}
    if right<=left||bottom<=top||right-left>16384||bottom-top>4096||(right-left)as u64*(bottom-top)as u64>16_000_000{return Err("Raster bounds outside probe limits".into())}let width=(right-left)as usize;let height=(bottom-top)as usize;let mut rgba=vec![0u8;width*height*4];
    let outline_rgb=[(input.font.outline_color>>16)as u8,(input.font.outline_color>>8)as u8,input.font.outline_color as u8];
    for(x,y_origin,g,part_color,page)in quads{let atlas=&pages[&page];let rgb=[(part_color>>16)as u8,(part_color>>8)as u8,part_color as u8];let sx=(g.uv[0]*atlas.size[0]as f32)as usize;let sy=(g.uv[2]*atlas.size[1]as f32)as usize;for y in 0..g.height as usize{for dx in 0..g.width as usize{let source=(sy+y)*atlas.size[0]as usize+sx+dx;if source>=atlas.alpha.len(){return Err("Glyph UV exceeds atlas".into())}let ink=atlas.alpha[source];let outline=atlas.outline.as_ref().map(|m|m[source]).unwrap_or(0);let coverage=(ink as u16+outline as u16).min(255)as u32;let a=(coverage*(part_color>>24)+127)/255;if a==0{continue}let target=((y+(y_origin-top)as usize)*width+(x-left)as usize+dx)*4;
        // gui_font shader: ceil(mask) color contribution, saturating ink+outline
        // coverage. Unit-scale integer quads sample each atlas texel at its center.
        let sa=a as f64/255.0;let da=rgba[target+3]as f64/255.0;let out_a=sa+da*(1.0-sa);
        for channel in 0..3{let color=(if ink>0{rgb[channel]as u16}else{0})+(if outline>0{outline_rgb[channel]as u16}else{0});rgba[target+channel]=((color.min(255)as f64*sa+rgba[target+channel]as f64*da*(1.0-sa))/out_a).round()as u8;}rgba[target+3]=(out_a*255.0).round()as u8;
    }}}
    let mut bytes=vec![];{let mut encoder=png::Encoder::new(&mut bytes,width as u32,height as u32);encoder.set_color(png::ColorType::Rgba);encoder.set_depth(png::BitDepth::Eight);let mut writer=encoder.write_header().map_err(|e|e.to_string())?;writer.write_image_data(&rgba).map_err(|e|e.to_string())?;}
    Ok(Raster{size:[width as u32,height as u32],origin:[left,top],advance:pen,png:format!("data:image/png;base64,{}",base64::engine::general_purpose::STANDARD.encode(bytes)),scope:"Own-process native atlas plus recovered unit-scale font shader equation and per-glyph shadow sequence. BMP pages through FE; page0 metadata verified, higher-page live metadata/pixels pending. No owner origin, fractional filtering or broad screenshot parity claim.".into()})
}
#[derive(Deserialize)]struct CapturedPage {#[serde(rename="Index")]index:u8,#[serde(rename="Glyphs")]glyphs:Vec<Glyph>}
#[derive(Deserialize)]struct CapturedFont {#[serde(flatten)]font:Font,#[serde(rename="CachedHeight")]cached_height:i32,#[serde(rename="Glyphs")]glyphs:Vec<Glyph>,#[serde(default,rename="Pages")]pages:Vec<CapturedPage>}
#[derive(Deserialize)]struct Capture {schema:u32,fonts:Vec<CapturedFont>}
#[derive(Serialize)]pub struct Row {pub name:String,pub exact:usize,pub mismatch:usize,pub reason:String}
#[derive(Serialize)]pub struct Report {pub exact:usize,pub mismatch:usize,pub unsupported:usize,pub rows:Vec<Row>,pub scope:String}
pub fn compare(json:&str)->Result<Report,String>{let capture:Capture=serde_json::from_str(json).map_err(|e|e.to_string())?;if capture.schema!=1{return Err("Expected glyph capture schema1".into())}
    let mut report=Report{exact:0,mismatch:0,unsupported:0,rows:vec![],scope:"Independent own-process atlas builder compared to captured already-loaded page width/height/advance/bearing/UV and font cached height. No captured glyph outputs used as builder inputs. This does not check GPU texture bytes or rendered pixels.".into()};
    for captured in capture.fonts{let mut row=Row{name:captured.font.name.clone(),exact:0,mismatch:0,reason:String::new()};match build(&captured.font){Err(e)=>{report.unsupported+=1;row.reason=e},Ok(atlas)=>{if atlas.cached_height!=captured.cached_height{row.reason=format!("Cached height {} != {}",atlas.cached_height,captured.cached_height);row.mismatch+=1;}
        if captured.glyphs.len()!=atlas.glyphs.len(){row.mismatch+=1;row.reason.push_str(" Glyph count differs.");}for(actual,predicted)in captured.glyphs.iter().zip(&atlas.glyphs){if actual==predicted{row.exact+=1}else{row.mismatch+=1;if row.reason.is_empty(){row.reason=format!("First differing glyph {}: captured {:?}; predicted {:?}",actual.code,actual,predicted);}}}
        for page in captured.pages{match build_page(&captured.font,page.index){Err(error)=>{report.unsupported+=1;row.reason.push_str(&format!(" Page{} unsupported: {error}",page.index));},Ok(predicted)=>{if page.glyphs.len()!=predicted.glyphs.len(){row.mismatch+=1;}for(actual,predicted)in page.glyphs.iter().zip(&predicted.glyphs){if actual==predicted{row.exact+=1}else{row.mismatch+=1;if row.reason.is_empty(){row.reason=format!("Page{} glyph{} differs",page.index,actual.code);}}}}}}
        report.exact+=row.exact;report.mismatch+=row.mismatch;}}report.rows.push(row);
    }Ok(report)
}
#[cfg(all(test,windows))]mod tests{
    use super::*;
    #[cfg(feature = "local-fidelity")]
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn headlines_opaque_pixels_match_independent_frame_crop(){
        use base64::Engine;
        fn decode(bytes:&[u8])->(png::OutputInfo,Vec<u8>){let mut decoder=png::Decoder::new(std::io::Cursor::new(bytes));decoder.set_transformations(png::Transformations::EXPAND|png::Transformations::STRIP_16);let mut reader=decoder.read_info().unwrap();let mut pixels=vec![0;reader.output_buffer_size()];let info=reader.next_frame(&mut pixels).unwrap();pixels.truncate(info.buffer_size());(info,pixels)}
        let input:RasterInput=serde_json::from_str(include_str!("../../fixtures/glyph-headlines-input.json")).unwrap();
        let predicted=render_run(&input).unwrap();assert_eq!(predicted.origin,[0,0]);assert_eq!(predicted.advance,90);
        let bytes=base64::engine::general_purpose::STANDARD.decode(predicted.png.split_once(',').unwrap().1).unwrap();
        let (actual_info,actual)=decode(include_bytes!("../../fixtures/glyph-headlines-reference.png"));let(predicted_info,pixels)=decode(&bytes);
        assert_eq!((actual_info.width,actual_info.height),(predicted_info.width,predicted_info.height));assert_eq!(actual_info.color_type,png::ColorType::Rgba);assert_eq!(predicted_info.color_type,png::ColorType::Rgba);
        let mut opaque=0;for (predicted,reference)in pixels.chunks_exact(4).zip(actual.chunks_exact(4)){if predicted[3]==255{opaque+=1;assert_eq!(&predicted[..3],&reference[..3]);}}assert_eq!(opaque,370);
        assert!(render_run(&RasterInput{font:input.font.clone(),text:"\u{ff00}".into(),color:input.color}).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn independent_captured_atlas_metadata_and_negative_uv(){
        let repo=crate::editor::discover_repo().unwrap();for name in ["glyph-metadata-live-1440x1080.json","glyph-metadata-heldout-live-1440x1080.json"]{
            let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures").join(name)).unwrap();let report=compare(&json).unwrap();assert_eq!((report.exact,report.mismatch,report.unsupported),(5632,0,0));
            let mut changed:serde_json::Value=serde_json::from_str(&json).unwrap();changed["fonts"][0]["Glyphs"][65]["UV"][0]=serde_json::json!(0.125);assert_eq!(compare(&changed.to_string()).unwrap().mismatch,1);
        }
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn extracted_xml_atlas_parameters_match_live_command_inputs(){
        let repo=crate::editor::discover_repo().unwrap();let json=std::fs::read_to_string(repo.join("tools/gui_editor/fixtures/glyph-metadata-live-1440x1080.json")).unwrap();let capture:Capture=serde_json::from_str(&json).unwrap();
        for name in ["Tooltip","Default","IngameMessages","Headlines","Mainmenu Header","Mainmenu Subheader","Mainmenu text black","CreditsHeader"]{let live=&capture.fonts.iter().find(|f|f.font.name==name).unwrap().font;for candidate in crate::localisation::inspect(&repo,"eng",-1,name).unwrap().fonts{let font=font_from_xml(&candidate.value,[1.0,1.0]).unwrap();assert_eq!((&font.face,font.width,font.height,font.weight,font.italic,font.quality,font.outline,font.underline),(&live.face,live.width,live.height,live.weight,live.italic,live.quality,live.outline,live.underline));assert_eq!(font.shadow,live.shadow);if font.shadow!=0{assert_eq!((font.shadow_color,font.shadow_x,font.shadow_y),(live.shadow_color,live.shadow_x,live.shadow_y));}if font.outline!=0{assert_eq!(font.outline_color,live.outline_color);}}}
        let mut invalid=capture.fonts[0].font.clone();invalid.width=i32::MIN;assert!(build(&invalid).is_err());assert!(font_from_xml("<Font/>",[f32::NAN,1.0]).is_err());
    }
    #[test]fn outline_color_order_and_unspecified_shadow_are_explicit(){
        let make=|body:&str|format!("<Font><FontName>Arial</FontName>{body}</Font>");
        assert_eq!(font_from_xml(&make("<OutlineBorderColor>123</OutlineBorderColor><Flag>Outline</Flag>"),[1.0,1.0]).unwrap().outline_color,0xff000000);
        assert_eq!(font_from_xml(&make("<Flag>Outline</Flag><OutlineBorderColor>123</OutlineBorderColor>"),[1.0,1.0]).unwrap().outline_color,123);
        assert!(font_from_xml(&make("<Flag>Shadow</Flag>"),[1.0,1.0]).is_err());
        assert!(font_from_xml(&make("<Flag>Shadow</Flag><ShadowColor>42242216120</ShadowColor>"),[1.0,1.0]).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn unicode_page_layout_and_bounds(){
        let xml="<Font><Name>Default</Name><FontName>Times New Roman</FontName><RelativeFontSize>18</RelativeFontSize><Flag>Antialiasing</Flag></Font>";
        let font=font_from_xml(xml,[1.0,1.0]).unwrap();let spec=crate::native_text::font_spec(xml).unwrap();
        let page=build_page(&font,0x20).unwrap();assert_eq!((page.glyphs[0].code,page.glyphs[255].code),(0x2000,0x20ff));
        let quote=&page.glyphs[0x19];assert_eq!(quote.advance,crate::native_text::measure(&spec,"’",[1.0,1.0]).unwrap().size[0]);
        let original="A’B";let raster=render_run(&RasterInput{font:font.clone(),text:original.into(),color:0xffffffff}).unwrap();let page0=build(&font).unwrap();assert_eq!(raster.advance,page0.glyphs[65].advance+quote.advance+page0.glyphs[66].advance);
        for text in ["\u{ff00}","😀","\n"]{assert!(render_run(&RasterInput{font:font.clone(),text:text.into(),color:0xffffffff}).is_err());}assert!(build_page(&font,255).is_err());
        let repo=crate::editor::discover_repo().unwrap();let mut editor=crate::editor::Editor::new(&repo).unwrap();let view=editor.open("menudefinitions/save_dialog.dlg").unwrap();let layout=editor.preview_layout(view.revision,1440,1080,false,&std::collections::BTreeMap::new()).unwrap();let result=crate::captions::preview(&repo,"eng",view.tree.as_ref().unwrap(),&layout);assert_eq!((result.captions.len(),result.sprites.len()),(3,9));assert!(result.captions.iter().any(|c|c.source_text.contains('…')));
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]fn document_retained_shadow_matches_both_captures(){
        let repo=crate::editor::discover_repo().unwrap();
        for fixture in ["glyph-metadata-live-1440x1080.json","glyph-metadata-heldout-live-1440x1080.json"]{
            let capture:Capture=serde_json::from_str(&std::fs::read_to_string(repo.join("tools/gui_editor/fixtures").join(fixture)).unwrap()).unwrap();
            let mut named_count=0;for captured in capture.fonts.iter().filter(|f|f.font.name!="MS Sanserif"){
                let live=&captured.font;let candidates=crate::localisation::inspect(&repo,"eng",-1,&live.name).unwrap().fonts;assert!(!candidates.is_empty());named_count+=1;
                for candidate in candidates{
                    let document=crate::localisation::font_document(&repo,"eng",&live.name,&candidate.source).unwrap();let predicted=font_from_document(&document,&live.name,[1.0,1.0]).unwrap();
                    assert_eq!((&predicted.face,predicted.height,predicted.width,predicted.weight,predicted.italic,predicted.underline,predicted.quality,predicted.outline,predicted.shadow),(&live.face,live.height,live.width,live.weight,live.italic,live.underline,live.quality,live.outline,live.shadow));
                    if predicted.shadow!=0{assert_eq!((predicted.shadow_color,predicted.shadow_x,predicted.shadow_y),(live.shadow_color,live.shadow_x,live.shadow_y));}if predicted.outline!=0{assert_eq!(predicted.outline_color,live.outline_color);}
                    assert_eq!(build(&predicted).unwrap().glyphs,captured.glyphs);
                }
            }assert_eq!(named_count,21);
            let live=&capture.fonts.iter().find(|f|f.font.name=="Mainmenu Subheader Start").unwrap().font;
            for candidate in crate::localisation::inspect(&repo,"eng",-1,&live.name).unwrap().fonts{
                assert!(font_from_xml(&candidate.value,[1.0,1.0]).is_err());
                let document=crate::localisation::font_document(&repo,"eng",&live.name,&candidate.source).unwrap();
                let predicted=font_from_document(&document,&live.name,[1.0,1.0]).unwrap();
                assert_eq!((predicted.shadow,predicted.shadow_x,predicted.shadow_y,predicted.shadow_color),(live.shadow,live.shadow_x,live.shadow_y,live.shadow_color));
                assert_eq!(predicted.shadow_color,15522472);
            }
        }
        let unknown="<Localisation><Font><Name>A</Name><FontName>Arial</FontName><Flag>Shadow</Flag></Font></Localisation>";assert!(font_from_document(unknown,"A",[1.0,1.0]).is_err());
        let reset="<Localisation><Font><Name>A</Name><ShadowColor>5</ShadowColor><ShadowOffset><x>4</x><y>7</y></ShadowOffset></Font><Font><Name>B</Name><FontName>Arial</FontName><Flag>Shadow</Flag></Font></Localisation>";let font=font_from_document(reset,"B",[1.0,1.0]).unwrap();assert_eq!((font.shadow_color,font.shadow_x,font.shadow_y),(5,0,0));
        assert!(crate::localisation::font_document(&repo,"eng","Tooltip","../../AGENTS.md").is_err());
    }
}
