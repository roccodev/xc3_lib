//! Map data stored in compressed sections in `.wismda` files.
//!
//! Many of these sections use the same formats as character models.

use binrw::{binread, FilePtr32};

use crate::{
    mxmd::{Materials, Models, TextureItems},
    parse_count_offset, parse_offset_count, parse_offset_count2, parse_string_ptr32,
    spch::Spch,
};

// TODO: Improve docs.
// TODO: Link to appropriate stream field with doc links.
/// The data for a [PropModel](crate::msmd::PropModel).
#[binread]
#[derive(Debug)]
pub struct PropModelData {
    pub unk1: [u32; 3],

    #[br(parse_with = FilePtr32::parse)]
    pub models: Models,

    #[br(parse_with = FilePtr32::parse)]
    pub materials: Materials,

    unk2: u32,

    // Is this the actual props in the scene?
    #[br(parse_with = FilePtr32::parse)]
    pub lods: PropLods,

    unk3: u32,

    /// The textures referenced by [materials](#structfield.materials).
    #[br(parse_with = parse_offset_count)]
    pub textures: Vec<Texture>,

    // TODO: lod def index -> prop_vertex_data_indices -> msmd prop_model_data
    // elements index into msmd prop_model_data?
    // something else indexes into this list?
    #[br(parse_with = parse_offset_count)]
    pub vertex_data_indices: Vec<u32>,

    unk4: [u32; 5],

    #[br(parse_with = FilePtr32::parse)]
    pub spch: Spch,

    unk5: u32,
    unk6: u32,
    // 16 bytes of padding?
}

// Similar to LOD data in mxmd?
#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct PropLods {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    unk1: u32,

    // Each of these is a single prop with all of its lods?
    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    pub props: Vec<PropLod>,

    count1: u32,
    offset1: u32,

    /// Instance information for [props](#structfield.props).
    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    pub instances: Vec<PropInstance>,

    count2: u32,
    offset2: u32,

    #[br(parse_with = parse_count_offset, args_raw(base_offset))]
    pub unk3: Vec<PropUnk3>,

    unks: [u32; 13],
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct PropLod {
    // start index into vertex_data_indices?
    // also start index into mesh.items?
    // TODO: Better name than mesh.items?
    pub base_lod_index: u32,
    pub lod_count: u32,
}

#[binread]
#[derive(Debug)]
pub struct PropInstance {
    /// The transform of the instance as a 4x4 column-major matrix.
    pub transform: [[f32; 4]; 4],

    unk2: [f32; 4],
    unk3: [f32; 3],

    // TODO: fix this doc link
    /// The index into [props](struct.PropLods.html#structfield.props).
    pub prop_index: u32,

    // padding?
    unk4: [u32; 4],
}

#[binread]
#[derive(Debug)]
pub struct PropUnk3 {
    unk1: [f32; 5],
    unk2: [u32; 3],
}

// TODO: Link to appropriate stream field with doc links.
/// The data for a [MapModel](crate::msmd::MapModel).
#[binread]
#[derive(Debug)]
pub struct MapModelData {
    unk1: [u32; 3],

    #[br(parse_with = FilePtr32::parse)]
    pub models: Models,

    #[br(parse_with = FilePtr32::parse)]
    pub materials: Materials,

    m_unk2: [u32; 2],

    /// The textures referenced by [materials](#structfield.materials).
    #[br(parse_with = parse_offset_count)]
    pub textures: Vec<Texture>,

    m_unk3: [u32; 2],

    #[br(parse_with = FilePtr32::parse)]
    pub spch: Spch,

    // TODO: What does this do?
    low_res_offset: u32,
    low_res_count: u32,

    #[br(parse_with = FilePtr32::parse)]
    pub mapping: UnkMapping,
    // padding?
}

// TODO: Shared with other formats?
#[binread]
#[derive(Debug)]
pub struct Texture {
    // TODO: What do these index into?
    pub low_texture_index: i16,
    pub low_texture_container_index: i16,
    pub texture_index: i16, // index into texture list in msmd?
    pub texture_type: u16,
}

// TODO: What to call this?
#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct UnkMapping {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub groups: Vec<UnkGroup>,

    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub indices: Vec<u16>,
}

// Groups?
#[binread]
#[derive(Debug)]
pub struct UnkGroup {
    max: [f32; 3],
    min: [f32; 3],
    // index for msmd map_model_data?
    // TODO: Sometimes out of bounds?
    pub vertex_data_index: u32,
    unk2: u32,
    unk3: u32,
}

// TODO: Where is the VertexData?
// TODO: Link to appropriate fields with doc links.
/// The data for a [SkyModel](crate::msmd::SkyModel).
#[binread]
#[derive(Debug)]
pub struct SkyModelData {
    #[br(parse_with = FilePtr32::parse)]
    pub models: Models,

    #[br(parse_with = FilePtr32::parse)]
    pub materials: Materials,

    // TODO: Pointers to MIBL files?
    unk_offset1: u32,
    unk_offset2: u32,

    #[br(parse_with = FilePtr32::parse)]
    textures: TextureItems,

    // TODO: always 0?
    unk6: u32,

    #[br(parse_with = FilePtr32::parse)]
    pub spch: Spch,
    // padding?
}

// TODO: Where is the VertexData?
// TODO: Link to appropriate fields with doc links.
/// The data for a [FoliageModel](crate::msmd::FoliageModel).
#[binread]
#[derive(Debug)]
pub struct FoliageModelData {
    #[br(parse_with = FilePtr32::parse)]
    pub models: Models,

    #[br(parse_with = FilePtr32::parse)]
    pub materials: FoliageMaterials,

    unk1: u32,
    unk2: u32,
    unk3: u32,
    unk4: [u32; 11], // padding?
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct FoliageMaterials {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_offset_count2, args_raw(base_offset))]
    pub materials: Vec<FoliageMaterial>,

    unk1: u32,
    unk2: u32,
    unk3: u32,
    unk4: u32,
    unk5: u32,
}

#[binread]
#[derive(Debug)]
#[br(import_raw(base_offset: u64))]
pub struct FoliageMaterial {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,

    unk1: u32,
    unk2: u32,
    unk3: u32, // textures?
    unk4: u32,
    unk5: u32,
    unk6: u32,
    unk7: u32,
}

#[binread]
#[derive(Debug)]
pub struct FoliageVertexData {
    #[br(parse_with = parse_count_offset)]
    unk1: Vec<FoliageVertex1>,
    #[br(parse_with = parse_count_offset)]
    unk2: Vec<FoliageVertex2>,
    unk3: u32,
    // TODO: padding?
    unks: [u32; 7],
}

#[binread]
#[derive(Debug)]
pub struct FoliageVertex1 {
    unk1: (f32, f32, f32),
    unk2: [u8; 4],
}

#[binread]
#[derive(Debug)]
pub struct FoliageVertex2 {
    unk1: (f32, f32, f32, f32),
    unk2: u32, // offset?
    unk3: u32, // offset?
    unk4: u32,
    unk5: u32,
}

#[binread]
#[derive(Debug)]
pub struct FoliageUnkData {
    unk1: [u32; 9], // length of the file repeated?
    unk2: [f32; 4],
    // TODO: padding?
    unk3: [u32; 8],
}

/// The data for a [UnkModel2](crate::msmd::UnkModel2).
#[binread]
#[derive(Debug)]
pub struct MapLowModelData {
    unk1: u32,
    unk2: u32,

    #[br(parse_with = FilePtr32::parse)]
    pub models: Models,

    #[br(parse_with = FilePtr32::parse)]
    pub materials: Materials,

    unk5: u32,
    unk6: u32,
    unk7: u32,
    unk8: u32,

    #[br(parse_with = FilePtr32::parse)]
    pub spch: Spch,
}

// TODO: Is this documented correctly?
// TODO: https://github.com/atnavon/xc2f/wiki/map-instance-chunk#extrainstancepack
#[binread]
#[derive(Debug)]
pub struct PropPositions {
    #[br(parse_with = parse_count_offset)]
    instances: Vec<Instance>,
    unk1: u32,
    unk2: u32,
    #[br(parse_with = parse_count_offset)]
    nodes: Vec<RenderNode>,
    unk3: u32,
    unk4: u32,
    unk5: u32,
    animated_parts_start_index: u32,
    animated_parts_count: u32,
    tree_offset: u32,
    unk6: u32,
    // TODO: more fields?
}

#[binread]
#[derive(Debug)]
pub struct Instance {
    matrix: [[f32; 4]; 4],
    positions: [f32; 3],
    radius: f32,
    center: [f32; 3],
    index: u32,
    unk1: u16,
    unk2: u16,
    unk3: u16,
    unk4: u16,
    // TODO: padding?
    unk: [u32; 2],
}

#[binread]
#[derive(Debug)]
pub struct RenderNode {
    center: [f32; 3],
    radius: f32,
    unk1: f32,
    unk2: u32,
    unk3: u32,
    unk4: u32,
}
