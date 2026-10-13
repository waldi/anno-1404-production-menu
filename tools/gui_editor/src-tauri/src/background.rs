//! Background grid rules recovered from Addon.exe; pixel parity remains unverified.
use base64::{engine::general_purpose::STANDARD, Engine};
use roxmltree::{Document, Node};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, String>;
#[derive(Clone, Debug, Serialize)]
pub struct Tile {
    pub source: [i32; 4],
    pub destination: [i32; 4],
    pub flags: i32,
    pub texture: String,
    pub repeat: [i32; 2],
}
#[derive(Serialize)]
pub struct Background {
    pub state: String,
    pub client_offsets: [i32; 4],
    pub tiles: Vec<Tile>,
    pub textures: BTreeMap<String, String>,
    pub sources:BTreeMap<String,String>,
    pub diagnostics: Vec<String>,
}
fn child<'a, 'b>(n: Node<'a, 'b>, name: &str) -> Option<Node<'a, 'b>> {
    n.children().find(|n| n.has_tag_name(name))
}
fn text(n: Node, name: &str) -> String {
    child(n, name)
        .and_then(|n| n.text())
        .unwrap_or("")
        .trim()
        .into()
}
fn int(n: Node, name: &str) -> Result<i32> {
    text(n, name)
        .parse()
        .map_err(|_| format!("Invalid background integer {name}"))
}

pub fn grid(xml: &str, state: &str, width: i32, height: i32) -> Result<Background> {
    if width <= 0 || height <= 0 || width > 8192 || height > 8192 {
        return Err("Background dimensions must be 1..8192".into());
    }
    let doc = Document::parse(xml).map_err(|e| e.to_string())?;
    let root = doc.root_element();
    let layouts = child(root, "BackgroundInfo").ok_or("Missing BackgroundInfo")?;
    let layout = layouts
        .children()
        .find(|n| n.is_element() && text(*n, "LayoutName") == state)
        .ok_or("Unknown background state; no guessed fallback")?;
    if int(layout, "flags")? != 0 {
        return Err("Layout flags not yet supported".into());
    }
    let x = int(layout, "m_XSize")? as usize;
    let y = int(layout, "m_YSize")? as usize;
    if x == 0 || y == 0 || x.checked_mul(y).is_none_or(|n| n > 100_000) {
        return Err("Invalid background grid".into());
    }
    let paths: Vec<_> = child(root, "m_localIconFileMap")
        .ok_or("Missing local texture map")?
        .children()
        .filter(|n| n.is_element())
        .map(|n| n.text().unwrap_or("").trim().replace('\\', "/"))
        .collect();
    let offsets = child(layout, "ClientAreaOffsets").ok_or("Missing client offsets")?;
    let mut tiles = vec![];
    for n in child(layout, "IconList")
        .ok_or("Missing IconList")?
        .children()
        .filter(|n| n.is_element())
    {
        if int(n, "isOldFormat")? != 0 {
            return Err("Old-format background icons need verified icon mapping".into());
        }
        let flags = int(n, "flags")?;
        if flags & !0x7f != 0 {
            return Err("Unknown tile flags".into());
        }
        let source = [
            int(n, "tileX")?,
            int(n, "tileY")?,
            int(n, "tileWidth")?,
            int(n, "tileHeight")?,
        ];
        if source.iter().any(|n| *n < 0) {
            return Err("Negative source rectangle".into());
        }
        let texture = paths
            .get(int(n, "LocalFileID")? as usize)
            .cloned()
            .or_else(|| {
                // Preserve allocation cells, but do not require an image for XML32
                // (engine skips draw) or a fixed zero-by-zero cell (no draw area).
                (flags & 32 != 0 || (flags == 0 && source[2] == 0 && source[3] == 0))
                    .then(String::new)
            })
            .ok_or("Invalid local texture index on drawable tile")?;
        let repeat = [int(n, "Width")?, int(n, "Height")?];
        if repeat.iter().any(|n| *n < 0 || *n > 255) {
            return Err("Invalid Width/Height multiplier".into());
        }
        tiles.push(Tile {
            source,
            destination: [0, 0, source[2], source[3]],
            flags,
            texture,
            repeat,
        });
    }
    if tiles.len() != x * y {
        return Err("Tile count does not match grid".into());
    }
    let first = tiles
        .iter()
        .find(|t| t.source[2] != 0 || t.source[3] != 0)
        .map(|t| t.destination)
        .unwrap_or([0; 4]);
    for t in &mut tiles {
        if t.flags & 32 != 0 {
            t.destination = first;
        }
    }
    for row in 0..y {
        let fixed: i32 = (0..x)
            .map(|col| &tiles[row * x + col])
            .filter(|t| t.flags & 5 == 0)
            .map(|t| t.destination[2])
            .sum();
        let flexible = (0..x)
            .filter(|col| tiles[row * x + col].flags & 5 != 0)
            .count() as i32;
        let mut left = 0;
        for col in 0..x {
            let t = &mut tiles[row * x + col];
            if t.flags & 5 != 0 {
                t.destination[2] = (width - fixed) / flexible;
            }
            t.destination[0] = left;
            left += t.destination[2];
        }
    }
    for col in 0..x {
        let fixed: i32 = (0..y)
            .map(|row| &tiles[row * x + col])
            .filter(|t| t.flags & 0x42 == 0)
            .map(|t| t.destination[3])
            .sum();
        let flexible = (0..y)
            .filter(|row| tiles[row * x + col].flags & 0x42 != 0)
            .count() as i32;
        let mut top = 0;
        for row in 0..y {
            let t = &mut tiles[row * x + col];
            if t.flags & 0x42 != 0 {
                t.destination[3] = (height - fixed) / flexible;
            }
            t.destination[1] = top;
            top += t.destination[3];
        }
    }
    for t in &mut tiles {
        t.destination[2] *= t.repeat[0] + 1;
        t.destination[3] *= t.repeat[1] + 1;
    }
    Ok(Background{state:state.into(),client_offsets:[int(offsets,"left")?,int(offsets,"top")?,int(offsets,"right")?,int(offsets,"bottom")?],tiles,textures:BTreeMap::new(),sources:BTreeMap::new(),diagnostics:vec!["Tile geometry confirmed static; texture filtering, blending, ABM and game pixel parity unverified.".into()]})
}

pub fn decode_dds(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let u = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    if bytes.len() < 128 || &bytes[..4] != b"DDS " || u(4) != 124 || u(76) != 32 {
        return Err("Invalid DDS header".into());
    }
    let (width, height) = (u(16), u(12));
    if width > 0
        && height > 0
        && width <= 8192
        && height <= 8192
        && u(80) & 4 != 0
    {
        let pixels=match &bytes[84..88] {b"DXT1"=>decode_bc12(&bytes[128..],width,height,false)?,b"DXT3"=>decode_bc12(&bytes[128..],width,height,true)?,b"DXT5"=>decode_bc3(&bytes[128..],width,height)?,_=>return Err(format!("Unsupported DDS FOURCC {}",String::from_utf8_lossy(&bytes[84..88])))};
        return Ok((width,height,pixels));
    }
    if width == 0
        || height == 0
        || width > 8192
        || height > 8192
        || u(80) & 0x40 == 0
        || u(80) & 4 != 0
        || u(88) != 32
    {
        return Err("Only bounded uncompressed 32-bit RGB DDS currently supported".into());
    }
    let stride = if u(8) & 8 != 0 {
        u(20) as usize
    } else {
        width as usize * 4
    };
    if stride < width as usize * 4 || bytes.len() < 128 + stride * height as usize {
        return Err("Truncated DDS pixels".into());
    }
    let masks = [u(92), u(96), u(100), u(104)];
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height as usize {
        for x in 0..width as usize {
            let p = u(128 + y * stride + x * 4);
            for (i, mask) in masks.iter().enumerate() {
                if *mask == 0 {
                    rgba.push(if i == 3 { 255 } else { 0 });
                    continue;
                }
                let shift = mask.trailing_zeros();
                let maximum = mask >> shift;
                if maximum & maximum.wrapping_add(1) != 0 {
                    return Err("Non-contiguous DDS channel mask".into());
                }
                rgba.push((((p & mask) >> shift) as u64 * 255 / maximum as u64) as u8);
            }
        }
    }
    Ok((width, height, rgba))
}
// Microsoft D3D9 DXT1 and explicit-alpha DXT3 specifications:
// https://learn.microsoft.com/en-us/windows/win32/direct3d9/opaque-and-1-bit-alpha-textures
// https://learn.microsoft.com/en-us/windows/win32/direct3d9/textures-with-alpha-channels
fn decode_bc12(bytes:&[u8],width:u32,height:u32,explicit_alpha:bool)->Result<Vec<u8>> {
    let columns=width.div_ceil(4) as usize;let rows=height.div_ceil(4) as usize;
    let block_size=if explicit_alpha{16}else{8};
    if bytes.len()<columns*rows*block_size{return Err("Truncated BC1/BC2 blocks".into())}
    let mut pixels=vec![0;width as usize*height as usize*4];
    for by in 0..rows {for bx in 0..columns {
        let block=&bytes[(by*columns+bx)*block_size..][..block_size];let color_offset=if explicit_alpha{8}else{0};let b=&block[color_offset..];
        let a=u16::from_le_bytes([b[0],b[1]]);let c=u16::from_le_bytes([b[2],b[3]]);
        let rgb=|v:u16|{let r=((v>>11)&31)as u8;let g=((v>>5)&63)as u8;let b=(v&31)as u8;[(r<<3)|(r>>2),(g<<2)|(g>>4),(b<<3)|(b>>2),255]};
        let mut colors=[[0u8;4];4];colors[0]=rgb(a);colors[1]=rgb(c);
        if explicit_alpha||a>c {
            for i in 0..3 {colors[2][i]=((2*colors[0][i]as u16+colors[1][i]as u16+1)/3)as u8;colors[3][i]=((colors[0][i]as u16+2*colors[1][i]as u16+1)/3)as u8;}
            colors[2][3]=255;colors[3][3]=255;
        }else{for i in 0..3{colors[2][i]=((colors[0][i]as u16+colors[1][i]as u16)/2)as u8;}colors[2][3]=255;}
        let indices=u32::from_le_bytes(b[4..8].try_into().unwrap());
        let alpha=if explicit_alpha{u64::from_le_bytes(block[..8].try_into().unwrap())}else{0};
        for i in 0..16 {let(x,y)=(bx*4+i%4,by*4+i/4);if x>=width as usize||y>=height as usize{continue}
            let mut color=colors[((indices>>(2*i))&3)as usize];if explicit_alpha{color[3]=((alpha>>(4*i))&15)as u8*17;}
            pixels[(y*width as usize+x)*4..][..4].copy_from_slice(&color);
        }
    }}
    Ok(pixels)
}
// BC3 block layout: Microsoft Direct3D block-compression documentation.
// https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression
fn decode_bc3(bytes: &[u8], width: u32, height: u32) -> Result<Vec<u8>> {
    let columns = width.div_ceil(4) as usize;
    let rows = height.div_ceil(4) as usize;
    if bytes.len() < columns * rows * 16 {
        return Err("Truncated BC3 blocks".into());
    }
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for by in 0..rows {
        for bx in 0..columns {
            let b = &bytes[(by * columns + bx) * 16..][..16];
            let mut alpha = [0u8; 8];
            alpha[0] = b[0];
            alpha[1] = b[1];
            let denominator = if alpha[0] > alpha[1] { 7 } else { 5 };
            for i in 1..denominator {
                alpha[i + 1] = (((denominator - i) as u16 * b[0] as u16 + i as u16 * b[1] as u16)
                    / denominator as u16) as u8;
            }
            if denominator == 5 {
                alpha[6] = 0;
                alpha[7] = 255;
            }
            let mut alpha_bits = 0u64;
            for i in 0..6 {
                alpha_bits |= (b[i + 2] as u64) << (i * 8);
            }
            let rgb = |v: u16| {
                let r = ((v >> 11) & 31) as u8;
                let g = ((v >> 5) & 63) as u8;
                let blue = (v & 31) as u8;
                [
                    (r << 3) | (r >> 2),
                    (g << 2) | (g >> 4),
                    (blue << 3) | (blue >> 2),
                ]
            };
            let mut colors = [[0u8; 3]; 4];
            colors[0] = rgb(u16::from_le_bytes([b[8], b[9]]));
            colors[1] = rgb(u16::from_le_bytes([b[10], b[11]]));
            for c in 0..3 {
                colors[2][c] = ((2 * colors[0][c] as u16 + colors[1][c] as u16) / 3) as u8;
                colors[3][c] = ((colors[0][c] as u16 + 2 * colors[1][c] as u16) / 3) as u8;
            }
            let indices = u32::from_le_bytes(b[12..16].try_into().unwrap());
            for i in 0..16 {
                let (x, y) = (bx * 4 + i % 4, by * 4 + i / 4);
                if x >= width as usize || y >= height as usize {
                    continue;
                }
                let offset = (y * width as usize + x) * 4;
                let color = colors[((indices >> (i * 2)) & 3) as usize];
                pixels[offset..offset + 3].copy_from_slice(&color);
                pixels[offset + 3] = alpha[((alpha_bits >> (i * 3)) & 7) as usize];
            }
        }
    }
    Ok(pixels)
}
pub fn texture_path(repo: &Path, logical: &str) -> Result<PathBuf> {
    if !(logical.starts_with("data/") || logical.starts_with("addondata/"))
        || logical.split('/').any(|p| p == ".." || p == "." || p.is_empty() || p.contains([':', '\\'])) {
        return Err("Invalid logical texture path".into());
    }
    let stem = logical
        .strip_suffix(".png")
        .ok_or("Expected nominal PNG texture path")?;
    let root = repo
        .join("data")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let mut candidates = vec![];
    for group in fs::read_dir(&root).map_err(|e| e.to_string())? {
        let group = group.map_err(|e| e.to_string())?;
        if !group.path().is_dir() {
            continue;
        }
        for archive in fs::read_dir(group.path()).map_err(|e| e.to_string())? {
            let archive = archive.map_err(|e| e.to_string())?;
            for suffix in [".dds", "_0.dds"] {
                let path = archive.path().join(format!("{stem}{suffix}"));
                if path.is_file() {
                    let path = path.canonicalize().map_err(|e| e.to_string())?;
                    if !path.starts_with(&root) {
                        return Err("Texture outside extracted root".into());
                    }
                    candidates.push(path);
                }
            }
        }
    }
    if candidates.len() != 1 {
        return Err(format!(
            "Texture {logical}: {} candidates; archive precedence not guessed",
            candidates.len()
        ));
    }
    Ok(candidates.remove(0))
}
pub fn render_data(
    repo: &Path,
    xml: &str,
    state: &str,
    width: i32,
    height: i32,
) -> Result<Background> {
    render_data_candidate(repo,xml,state,width,height,None)
}
pub fn render_data_candidate(repo:&Path,xml:&str,state:&str,width:i32,height:i32,archive:Option<&str>)->Result<Background>{
    if archive.is_some_and(|a|!matches!(a,"maindata/patch0.rda"|"maindata/patch1.rda"|"startup-eng")){return Err("Unknown artwork profile".into())}
    let mut result = grid(xml, state, width, height)?;
    if let Some(archive)=archive{result.diagnostics.push(if archive=="startup-eng"{"Known English default startup order across available extracted/staged assets; loose files and later custom mounts are not modeled.".into()}else{format!("Explicit {archive} artwork candidate where staged; other textures require a unique extracted source. Effective live file origin is unverified.")});}
    for tile in &result.tiles {
        if tile.flags & 32 != 0 || tile.destination[2] <= 0 || tile.destination[3] <= 0 {
            continue;
        }
        if result.textures.contains_key(&tile.texture) {
            continue;
        }
        let path=texture_path_candidate(repo,&tile.texture,archive)?;
        result.sources.insert(tile.texture.clone(),path.strip_prefix(repo).unwrap_or(&path).to_string_lossy().replace('\\',"/"));
        let bytes = fs::read(path).map_err(|e| e.to_string())?;
        let (w, h, pixels) = decode_dds(&bytes)?;
        for tile in result.tiles.iter().filter(|t| t.texture == tile.texture) {
            if tile.source[0] + tile.source[2] > w as i32
                || tile.source[1] + tile.source[3] > h as i32
            {
                return Err("Tile source exceeds texture".into());
            }
        }
        let mut png = vec![];
        {
            let mut encoder = png::Encoder::new(&mut png, w, h);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .map_err(|e| e.to_string())?
                .write_image_data(&pixels)
                .map_err(|e| e.to_string())?;
        }
        result.textures.insert(
            tile.texture.clone(),
            format!("data:image/png;base64,{}", STANDARD.encode(png)),
        );
    }
    Ok(result)
}
pub fn texture_path_candidate(repo:&Path,logical:&str,archive:Option<&str>)->Result<PathBuf>{
    let Some(archive)=archive else{return texture_path(repo,logical)};
    if !matches!(archive,"maindata/patch0.rda"|"maindata/patch1.rda"|"startup-eng"){return Err("Unknown artwork profile".into())}
    let stem=logical.strip_suffix(".png").ok_or("Expected nominal PNG texture")?;
    if !(logical.starts_with("data/")||logical.starts_with("addondata/"))||stem.split('/').any(|p|p==".."||p=="."||p.is_empty()||p.contains([':', '\\'])){return Err("Invalid logical texture path".into())}
    let root=repo.join("tools/gui_editor/fixtures/archive-candidates").canonicalize().map_err(|e|e.to_string())?;
    if archive=="startup-eng"{return startup_texture_path(repo,stem,&root)}
    for source_root in [&root, &repo.join("data").canonicalize().map_err(|e|e.to_string())?] {
        for suffix in ["_0.dds", ".dds"] {
            let candidate=source_root.join(archive).join(format!("{stem}{suffix}"));
            if candidate.is_file(){let candidate=candidate.canonicalize().map_err(|e|e.to_string())?;if !candidate.starts_with(source_root){return Err("Candidate outside artwork root".into())}return Ok(candidate)}
        }
    }
    texture_path(repo,logical)
}
fn startup_rank(archive:&str)->Option<u8>{
    let (group,name)=archive.split_once('/')?;
    if let Some(index)=name.strip_prefix("data").and_then(|n|n.strip_suffix(".rda")).and_then(|n|n.parse::<u8>().ok()).filter(|n|*n<6){return match group{"maindata"=>Some(index),"addon"=>Some(9+index),_=>None}}
    match archive{"maindata/eng0.rda"=>Some(6),"maindata/patch0.rda"=>Some(7),"maindata/patch1.rda"=>Some(8),"addon/eng0.rda"=>Some(15),"addon/patch0.rda"=>Some(16),_=>None}
}
fn startup_texture_path(repo:&Path,stem:&str,staged:&Path)->Result<PathBuf>{
    let data=repo.join("data").canonicalize().map_err(|e|e.to_string())?;
    let mut candidates:BTreeMap<(u8,String),PathBuf>=BTreeMap::new();
    for root in [&data,staged]{
        for group in fs::read_dir(root).map_err(|e|e.to_string())?{
            let group=group.map_err(|e|e.to_string())?;if !group.path().is_dir(){continue}
            for archive in fs::read_dir(group.path()).map_err(|e|e.to_string())?{
                let archive=archive.map_err(|e|e.to_string())?;if !archive.path().is_dir(){continue}
                let key=format!("{}/{}",group.file_name().to_string_lossy(),archive.file_name().to_string_lossy()).to_ascii_lowercase();let Some(rank)=startup_rank(&key)else{continue};
                for suffix in [".dds","_0.dds"]{
                    let path=archive.path().join(format!("{stem}{suffix}"));if !path.is_file(){continue}
                    let path=path.canonicalize().map_err(|e|e.to_string())?;if !path.starts_with(root){return Err("Texture escaped artwork root".into())}
                    let candidate_key=(rank,key.clone());
                    if let Some(previous)=candidates.get(&candidate_key){
                        if previous.file_name()!=path.file_name()||fs::read(previous).map_err(|e|e.to_string())?!=fs::read(&path).map_err(|e|e.to_string())?{return Err(format!("Ambiguous texture representations in {key}"))}
                    }else{candidates.insert(candidate_key,path);}
                }
            }
        }
    }
    candidates.into_iter().next_back().map(|(_,path)|path).ok_or("No texture in available English startup archives".into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn english_startup_profile_selects_verified_available_archive_candidates() {
        let repo=crate::editor::discover_repo().unwrap();
        for (logical,archive) in [
            ("data/graphics/ui/textures/ingame_gui_map_03.png","maindata/patch1.rda"),
            ("data/graphics/ui/textures/main_gui_map_01.png","maindata/patch0.rda"),
            ("addondata/graphics/ui/textures/ingame_gui_map_05.png","addon/data2.rda"),
            ("data/graphics/ui/icons/gui_icon_map_anno4_sorted.png","maindata/patch1.rda"),
            ("data/loca/eng/textures/anno_logo.png","maindata/eng0.rda")
        ]{
            let selected=texture_path_candidate(&repo,logical,Some("startup-eng")).unwrap();
            assert!(selected.to_string_lossy().replace('\\',"/").contains(archive),"{logical}: {}",selected.display());
        }
        assert_eq!(startup_rank("maindata/patch2.rda"),None);assert_eq!(startup_rank("addon/data6.rda"),None);
        assert!(startup_rank("addon/data0.rda")>startup_rank("maindata/patch1.rda"));
        for path in ["data/../escape.png","data/C:/escape.png","addondata/\\escape.png"]{assert!(texture_path_candidate(&repo,path,Some("startup-eng")).is_err())}
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn archived_patch_candidates_match_captured_opaque_button_pixels_without_fitting() {
        let repo=crate::editor::discover_repo().unwrap();
        let xml=fs::read_to_string(repo.join("data/addon/data1.rda/addondata/config/gui/backgrounds/final/minimap_frames/force_north_button.bkg")).unwrap();
        for reference_file in ["force-north-checked-live.png","force-north-checked-visible-heldout-live.png"] {
        let mut decoder=png::Decoder::new(std::io::Cursor::new(fs::read(repo.join("tools/gui_editor/fixtures").join(reference_file)).unwrap()));
        decoder.set_transformations(png::Transformations::EXPAND|png::Transformations::STRIP_16);
        let mut reader=decoder.read_info().unwrap();let mut reference=vec![0;reader.output_buffer_size()];let info=reader.next_frame(&mut reference).unwrap();
        assert_eq!((info.width,info.height),(24,24));let channels=match info.color_type{png::ColorType::Rgb=>3,png::ColorType::Rgba=>4,_=>panic!("Unexpected reference channels")};
        for archive in ["maindata/patch0.rda","maindata/patch1.rda"] {
            let preview=render_data_candidate(&repo,&xml,"Checked",24,24,Some(archive)).unwrap();
            let bytes=fs::read(repo.join("tools/gui_editor/fixtures/archive-candidates").join(archive).join("data/graphics/ui/textures/ingame_gui_map_03_0.dds")).unwrap();
            let (width,_,pixels)=decode_dds(&bytes).unwrap();let mut count=0;let mut maximum=0;
            for tile in preview.tiles {
                let [sx,sy,w,h]=tile.source;let [dx,dy,dw,dh]=tile.destination;assert_eq!((dw,dh),(w,h));
                for y in 0..h{for x in 0..w{let source=((sy+y)as usize*width as usize+(sx+x)as usize)*4;if pixels[source+3]!=255{continue}let target=((dy+y)as usize*24+(dx+x)as usize)*channels;for c in 0..3{maximum=maximum.max(pixels[source+c].abs_diff(reference[target+c]));}count+=1}}
            }
            assert_eq!(count,122);assert!(maximum<=1,"{archive}: maximum error {maximum}");
        }
        }
        assert!(render_data_candidate(&repo,&xml,"Checked",24,24,Some("../escape")).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn loaded_checked_button_tiles_match_source_allocation_and_logical_texture() {
        let repo=crate::editor::discover_repo().unwrap();
        let fixture:serde_json::Value=serde_json::from_str(&fs::read_to_string(repo.join("tools/gui_editor/fixtures/force-north-loaded-background.json")).unwrap()).unwrap();
        let xml=fs::read_to_string(repo.join("data/addon/data1.rda/addondata/config/gui/backgrounds/final/minimap_frames/force_north_button.bkg")).unwrap();
        let preview=grid(&xml,"Checked",24,24).unwrap();
        let live=fixture["metadata"]["tiles"].as_array().unwrap();assert_eq!(live.len(),4);assert_eq!(preview.tiles.len(),live.len());
        for (tile,actual) in preview.tiles.iter().zip(live){
            assert_eq!(serde_json::json!(tile.source),actual["source"]);
            let [x,y,w,h]=tile.destination;assert_eq!(serde_json::json!([x,y,x+w,y+h]),actual["destination"]);
            assert_eq!(tile.texture,actual["filename"].as_str().unwrap());assert_eq!(tile.flags,actual["flags"].as_i64().unwrap()as i32);
        }
        // Captured renderer handles and remapped FileIDs are deliberately not preview inputs.
        assert_eq!(fixture["metadata"]["layout_color"],serde_json::json!(u32::MAX));
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn non_drawing_cells_preserve_grid_without_requiring_invalid_texture_ids() {
        let repo=crate::editor::discover_repo().unwrap();
        let xml=fs::read_to_string(repo.join("data/addon/data1.rda/addondata/config/gui/backgrounds/final/bailiwick_frames/bailiwickcheckbox.bkg")).unwrap();
        let bg=grid(&xml,"Normal+MouseButtonDown",100,50).unwrap();
        assert_eq!(bg.tiles.len(),140);
        assert!(bg.tiles.iter().any(|t|t.texture.is_empty()));
        assert!(bg.tiles.iter().filter(|t|t.texture.is_empty()).all(|t|t.flags&32!=0||t.destination[2]==0&&t.destination[3]==0));
        let bad=xml.replacen("<tileWidth>0</tileWidth>","<tileWidth>4</tileWidth>",1).replacen("<tileHeight>0</tileHeight>","<tileHeight>4</tileHeight>",1);
        assert!(grid(&bad,"Normal+MouseButtonDown",100,50).is_err());
        for path in ["addondata/../escape.png","addondata/C:/escape.png","addondata/./escape.png","addondata/\\escape.png"] {assert!(texture_path(&repo,path).is_err());}
    }
    #[test]
    fn bc1_transparency_bc2_explicit_alpha_and_four_color_override() {
        let mut b=[0u8;16];b[8..10].copy_from_slice(&0x001fu16.to_le_bytes());b[10..12].copy_from_slice(&0xf800u16.to_le_bytes());b[12..16].copy_from_slice(&0xffffffffu32.to_le_bytes());
        assert_eq!(&decode_bc12(&b[8..],1,1,false).unwrap(),&[0,0,0,0]);
        b[..8].copy_from_slice(&0x76543210fedcba98u64.to_le_bytes());
        let p=decode_bc12(&b,4,4,true).unwrap();
        assert_eq!(&p[..4],&[170,0,85,136]);assert_eq!(p[7],153);assert_eq!(p[63],119);
        assert!(decode_bc12(&b[..15],4,4,true).is_err());
        b[8..10].copy_from_slice(&0xf800u16.to_le_bytes());b[10..12].copy_from_slice(&0u16.to_le_bytes());b[12..16].copy_from_slice(&0xaaaaaaaau32.to_le_bytes());
        assert_eq!(&decode_bc12(&b[8..],1,1,false).unwrap(),&[170,0,0,255]);
    }
    #[test]
    fn bc3_alpha_branches_colors_and_partial_blocks() {
        let mut block = [0u8; 16];
        block[0] = 255;
        block[1] = 0;
        // alpha index2 => (6*255+0)/7; color index2 => 2/3 red + 1/3 blue.
        let mut bits = 0u64;
        for i in 0..16 {
            bits |= 2u64 << (i * 3);
        }
        for i in 0..6 {
            block[i + 2] = (bits >> (i * 8)) as u8;
        }
        block[8..10].copy_from_slice(&0xf800u16.to_le_bytes());
        block[10..12].copy_from_slice(&0x001fu16.to_le_bytes());
        block[12..].copy_from_slice(&0xaaaaaaaau32.to_le_bytes());
        let p = decode_bc3(&block, 3, 2).unwrap();
        assert_eq!(p.len(), 24);
        assert_eq!(&p[..4], &[170, 0, 85, 218]);
        block[0] = 0;
        block[1] = 255;
        bits = 0;
        for i in 0..16 {
            bits |= 7u64 << (i * 3);
        }
        for i in 0..6 {
            block[i + 2] = (bits >> (i * 8)) as u8;
        }
        assert_eq!(decode_bc3(&block, 4, 4).unwrap()[3], 255);
        bits = 0;
        for i in 0..16 {
            bits |= 6u64 << (i * 3);
        }
        for i in 0..6 {
            block[i + 2] = (bits >> (i * 8)) as u8;
        }
        assert_eq!(decode_bc3(&block, 4, 4).unwrap()[3], 0);
        assert!(decode_bc3(&block[..15], 4, 4).is_err());
    }
    #[test]
    #[ignore = "local fidelity suite: requires game resources or captured fixtures"]
    fn objectmenu_grid() {
        let repo = crate::editor::discover_repo().unwrap();
        let xml=fs::read_to_string(repo.join("data/addon/data1.rda/addondata/config/gui/backgrounds/final/objectmenu_frames/objectmenuinside.bkg")).unwrap();
        let b = grid(&xml, "EmptyLayout", 400, 100).unwrap();
        assert_eq!(b.tiles.len(), 35);
        assert_eq!(b.client_offsets, [0, 0, 0, 12]);
        assert_eq!(b.tiles[3].destination, [36, 0, 328, 12]);
        assert_eq!(b.tiles[17].destination, [36, 24, 328, 52]);
        assert_eq!(b.tiles[34].destination, [388, 88, 12, 12]);
        assert!(grid(&xml, "Missing", 400, 100).is_err());
        let data = render_data(&repo, &xml, "EmptyLayout", 400, 100).unwrap();
        assert_eq!(data.textures.len(), 1);
    }
    #[test]
    fn dds_masks_and_truncation() {
        let mut b = vec![0u8; 132];
        b[..4].copy_from_slice(b"DDS ");
        for (o, v) in [
            (4, 124u32),
            (76, 32),
            (16, 1),
            (12, 1),
            (80, 0x41),
            (88, 32),
            (92, 0xff0000),
            (96, 0xff00),
            (100, 0xff),
            (104, 0xff000000),
            (128, 0x80402010),
        ] {
            b[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        assert_eq!(decode_dds(&b).unwrap().2, vec![0x40, 0x20, 0x10, 0x80]);
        assert!(decode_dds(&b[..131]).is_err());
        assert!(decode_dds(b"DDS ").is_err());
    }
}
