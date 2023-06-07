use std::io::SeekFrom;

use crate::{parse_count_offset, parse_offset_count, parse_ptr32};
use binrw::{args, binread, BinRead, BinResult, FilePtr32, NamedArgs, NullString};
use serde::Serialize;

/// .wimdo files
#[binread]
#[derive(Debug, Serialize)]
#[br(magic(b"DMXM"))]
pub struct Mxmd {
    version: u32,

    #[br(parse_with = FilePtr32::parse)]
    pub mesh: Mesh,

    #[br(parse_with = FilePtr32::parse)]
    pub materials: Materials,

    #[br(parse_with = parse_ptr32)]
    unk1: Option<Unk1>,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    unk5: u32,

    // uncached textures?
    #[br(parse_with = FilePtr32::parse)]
    pub textures: Textures,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct Materials {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(args { base_offset, inner: base_offset })]
    pub materials: List<Material>,

    unk1: u32,
    unk2: u32,

    // TODO: Materials have offsets into these arrays for parameter values?
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    floats: Vec<f32>,

    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    ints: Vec<u32>,

    // TODO: what type is this?
    unk_offset1: u32, // offset?
    unk4: u32,

    // TODO: How large is each element?
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    unks: Vec<[u16; 8]>,

    unks1: [u32; 2],

    // array of (u32, u32)?
    unk_count: u32,
    unk_offset2: u32,

    unks2: [u32; 7],

    unk_offset3: u32,

    unks3: [u32; 4],
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct Material {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,

    unk1: u16,
    unk2: u16,
    unk3: u16,
    unk4: u16,

    /// Color multiplier value assigned to the `gMatCol` shader uniform.
    pub color: [f32; 4],

    unk_float: f32,

    // TODO: materials with zero textures?
    /// Defines the shader's sampler bindings in order for s0, s1, s2, ...
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub textures: Vec<Texture>,

    pub unk_flag1: [u8; 4],
    // stencil stuff,
    // 1 = enable stencil test?
    // changes pass and shader, 0=no depth?, normal=1, ope=3?
    // ???
    pub unk_flag2: [u8; 4],

    m_unks1: [u32; 6],

    m_unk5: u32,

    // always count 1?
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub shader_programs: Vec<ShaderProgram>,

    m_unks2: [u16; 16],
}

#[binread]
#[derive(Debug, Serialize)]
pub struct ShaderProgram {
    pub program_index: u32, // index into programs in wismt?
    pub unk_type: ShaderUnkType,
    pub parent_material_index: u16, // index of the parent material?
    pub unk4: u32,                  // always 1?
}

// Affects what pass the object renders in?
// Each "pass" has different render targets?
// _trans = 1,
// _ope = 0,1,7
// _zpre = 0
// _outline = 0
#[binread]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize)]
#[br(repr(u16))]
pub enum ShaderUnkType {
    Unk0 = 0, // main opaque + some transparent?
    Unk1 = 1, // second layer transparent?
    Unk7 = 7, // additional eye effect layer?
}

#[binread]
#[derive(Debug, Serialize)]
pub struct Texture {
    pub texture_index: u16,
    pub unk1: u16, // sampler index?
    pub unk2: u16,
    pub unk3: u16,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct Mesh {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    unk1: u32,

    max_xyz: [f32; 3],
    min_xyz: [f32; 3],

    #[br(args { base_offset, inner: base_offset })]
    pub items: List<DataItem>,

    unk2: u32,

    #[br(parse_with = parse_ptr32, args_raw(base_offset))]
    skeleton: Option<Skeleton>,

    unks3: [u32; 22],

    #[br(parse_with = parse_ptr32, args_raw(base_offset))]
    pub unk_offset1: Option<MeshUnk1>,

    unk_offset2: u32,

    #[br(parse_with = parse_ptr32, args_raw(base_offset))]
    lod_data: Option<LodData>,
}

// TODO: Better names for these types
#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct DataItem {
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub sub_items: Vec<SubDataItem>,

    unk1: u32,
    max_xyz: [f32; 3],
    min_xyz: [f32; 3],
    bounding_radius: f32,
    unks: [u32; 7],
}

// TODO: Better names for these types
#[binread]
#[derive(Debug, Serialize)]
pub struct SubDataItem {
    flags1: u32,
    flags2: u32,
    pub vertex_buffer_index: u16,
    pub index_buffer_index: u16,
    unk_index: u16,
    pub material_index: u16,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    unk5: u16,
    pub lod: u16,
    // TODO: groups?
    unks6: [i32; 4],
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct MeshUnk1 {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = FilePtr32::parse)]
    #[br(args { offset: base_offset, inner: base_offset })]
    pub inner: MeshUnk1Inner,
    unk1: [u32; 14],
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct MeshUnk1Inner {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub unk1: String,

    unk2: [f32; 9],
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct LodData {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    unk1: u32,

    // another list?
    unk2: u32,
    unk3: u32,

    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    items: Vec<(u16, u16)>,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct Textures {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    unks: [u32; 5],

    unk_offset: u32, // 292 bytes?

    unks2: [u32; 8],

    #[br(parse_with = FilePtr32::parse, offset = base_offset)]
    unk2: [u32; 7],

    #[br(parse_with = parse_ptr32, args_raw(base_offset))]
    pub items: Option<TextureItems>,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct TextureItems {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    count: u32,
    offset: u32,
    unk2: u32,
    strings_offset: u32,

    #[br(args { count: count as usize, inner: args! { base_offset } })]
    pub textures: Vec<TextureItem>,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import { base_offset: u64 })]
pub struct TextureItem {
    unk1: u16,
    unk2: u16,
    unk3: u16, // size?
    unk4: u16,
    unk5: u16, // some sort of offset (sum of previous unk3)?
    unk6: u16,

    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct Skeleton {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    count1: u32,
    count2: u32,

    // TODO: Find a simpler way of writing this?
    #[br(parse_with = FilePtr32::parse)]
    #[br(args {
        offset: base_offset,
        inner: args! {
            count: count1 as usize,
            inner: base_offset
        }
    })]
    bones: Vec<Bone>,

    // TODO: Create a matrix type?
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { offset: base_offset, inner: args! { count: count1 as usize } })]
    transforms: Vec<[[f32; 4]; 4]>,

    unk_offset1: u32,
    unk_offset2: u32,
    count3: u32,
    unk_offset3: u32,
    unk_offset4: u32,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct Bone {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    name: String,
    unk1: f32,
    unk_type: u32,
    #[br(pad_after = 8)]
    unk_index: u32,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct Unk1 {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    unk1: Vec<Unk1Unk1>,

    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    unk2: Vec<Unk1Unk2>,

    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    unk3: Vec<Unk1Unk3>,

    // angle values?
    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    unk4: Vec<Unk1Unk4>,
}

#[binread]
#[derive(Debug, Serialize)]
pub struct Unk1Unk1 {
    index: u16,
    unk2: u16, // 1
}

#[binread]
#[derive(Debug, Serialize)]
pub struct Unk1Unk2 {
    unk1: u16, // 0
    index: u16,
    unk3: u16,
    unk4: u16,
    unk5: u32, // 0
}

#[binread]
#[derive(Debug, Serialize)]
pub struct Unk1Unk3 {
    unk1: u16,
    unk2: u16,
    unk3: u32,
    unk4: u16,
    unk5: u16,
    unk6: u16,
    unk7: u16,
}

#[binread]
#[derive(Debug, Serialize)]
pub struct Unk1Unk4 {
    unk1: f32,
    unk2: f32,
    unk3: f32,
    unk4: u32,
}

// TODO: shared with hpcs?
fn parse_string_ptr32<R: std::io::Read + std::io::Seek>(
    reader: &mut R,
    endian: binrw::Endian,
    args: (u64,),
) -> BinResult<String> {
    let offset = u32::read_options(reader, endian, ())?;
    let saved_pos = reader.stream_position()?;

    reader.seek(SeekFrom::Start(args.0 + offset as u64))?;
    let value = NullString::read_options(reader, endian, ())?;
    reader.seek(SeekFrom::Start(saved_pos))?;

    Ok(value.to_string())
}

/// A [u32] offset and [u32] count with an optional base offset.
#[derive(Clone, NamedArgs)]
pub struct ListArgs<Inner: Default> {
    #[named_args(default = 0)]
    base_offset: u64,
    #[named_args(default = Inner::default())]
    inner: Inner,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(args: ListArgs<T::Args<'_>>))]
#[serde(transparent)]
pub struct List<T>
where
    T: BinRead + 'static,
    for<'a> <T as BinRead>::Args<'a>: Clone + Default,
{
    #[br(temp)]
    offset: u32,
    #[br(temp)]
    count: u32,

    #[br(args { count: count as usize, inner: args.inner })]
    #[br(seek_before = SeekFrom::Start(args.base_offset + offset as u64))]
    #[br(restore_position)]
    pub elements: Vec<T>,
}
