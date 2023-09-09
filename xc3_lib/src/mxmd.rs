//! Model data in `.wimdo` files.
use crate::{
    msrd::TextureResource, parse_count_offset, parse_offset_count, parse_opt_ptr32, parse_ptr32,
    parse_string_ptr32, spch::Spch, vertex::VertexData, write::Xc3Write,
};
use bilge::prelude::*;
use binrw::{args, binread, BinRead};

/// .wimdo files
#[derive(BinRead, Debug)]
#[br(magic(b"DMXM"))]
pub struct Mxmd {
    pub version: u32,

    #[br(parse_with = parse_ptr32)]
    pub models: Models,

    #[br(parse_with = parse_ptr32)]
    pub materials: Materials,

    #[br(parse_with = parse_opt_ptr32)]
    pub unk1: Option<Unk1>,

    /// Embedded vertex data for .wimdo only models with no .wismt.
    #[br(parse_with = parse_opt_ptr32)]
    pub vertex_data: Option<VertexData>,

    /// Embedded shader data for .wimdo only models with no .wismt.
    #[br(parse_with = parse_opt_ptr32)]
    pub spch: Option<Spch>,

    #[br(parse_with = parse_opt_ptr32)]
    pub packed_textures: Option<PackedTextures>,

    pub unk5: u32,

    // unpacked textures?
    #[br(parse_with = parse_opt_ptr32)]
    pub textures: Option<Textures>,
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Materials {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_offset_count, args { offset: base_offset, inner: base_offset })]
    pub materials: Vec<Material>,

    // offset?
    pub unk1: u32,
    pub unk2: u32,

    // TODO: Materials have offsets into these arrays for parameter values?
    // material body has a uniform at shader offset 64 but offset 48 in this floats buffer
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub floats: Vec<f32>, // work values?

    // TODO: final number counts up from 0?
    // TODO: Some sort of index or offset?
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub ints: Vec<(u8, u8, u16)>, // shader vars?

    #[br(parse_with = parse_ptr32)]
    #[br(args { offset: base_offset, inner: base_offset })]
    pub unk_offset1: MaterialUnk1, // callbacks?

    // TODO: is this ever not 0?
    pub unk4: u32,

    /// Info for each of the shaders in the associated [Spch](crate::spch::Spch).
    #[br(parse_with = parse_offset_count, args { offset: base_offset, inner: base_offset })]
    pub shader_programs: Vec<ShaderProgramInfo>,

    pub unks1: [u32; 2],

    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub alpha_test_textures: Vec<AlphaTestTexture>,

    pub unks3: [u32; 7],

    #[br(parse_with = parse_opt_ptr32, offset = base_offset)]
    pub samplers: Option<Samplers>,

    // TODO: padding?
    pub unks4: [u32; 4],
}

#[derive(BinRead, Debug)]
pub struct AlphaTestTexture {
    // TODO: (_, 0, 1) has alpha testing?
    // TODO: Test different param values?
    pub texture_index: u16,
    pub unk1: u16,
    pub unk2: u32,
}

/// `ml::MdsMatTechnique` in the Xenoblade 2 binary.
#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct ShaderProgramInfo {
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub unk1: Vec<u64>, // vertex attributes?

    pub unk3: u32, // 0
    pub unk4: u32, // 0

    // work values?
    // TODO: matches up with uniform parameters for U_Mate?
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub parameters: Vec<MaterialParameter>, // var table?

    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub textures: Vec<u16>, // textures?

    // ssbos and then uniform buffers ordered by handle?
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub uniform_blocks: Vec<(u16, u16)>, // uniform blocks?

    pub unk11: u32, // material texture count?

    pub unk12: u16, // counts up from 0?
    pub unk13: u16, // unk11 + unk12?

    // TODO: padding?
    pub padding: [u32; 5],
}

/// `ml::MdsMatVariableTbl` in the Xenoblade 2 binary.
#[derive(BinRead, Debug)]
pub struct MaterialParameter {
    pub param_type: ParamType,
    pub floats_index_offset: u16, // added to floats start index?
    pub unk: u16,
    pub count: u16, // actual number of bytes depends on type?
}

#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[br(repr(u16))]
pub enum ParamType {
    Unk0 = 0,
    /// `gTexMat` uniform in the [Spch] and
    /// `ml::DrMdoSetup::unimate_texMatrix` in the Xenoblade 2 binary.
    TexMatrix = 1,
    /// `gWrkFl4[0]` uniform in the [Spch] and
    /// `ml::DrMdoSetup::unimate_workFloat4` in the Xenoblade 2 binary.
    WorkFloat4 = 2,
    /// `gWrkCol` uniform in the [Spch] and
    /// `ml::DrMdoSetup::unimate_workColor` in the Xenoblade 2 binary.
    WorkColor = 3,
    Unk4 = 4,
    /// `gAlInf` uniform in the [Spch] and
    /// `ml::DrMdoSetup::unimate_alphaInfo` in the Xenoblade 2 binary.
    Unk5 = 5,
    Unk6 = 6,
    Unk7 = 7,
    /// `gToonHeadMat` uniform in the [Spch].
    Unk10 = 10,
}

// TODO: Does this affect texture assignment order?
#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct MaterialUnk1 {
    // count matches up with Material.unk_start_index?
    // TODO: affects material parameter assignment?
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub unk1: Vec<(u16, u16)>,

    // 0 1 2 ... material_count - 1
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub unk2: Vec<u16>,
}

#[derive(BinRead, Debug)]
pub struct Samplers {
    pub unk1: u32, // count?
    pub unk2: u32, // offset?
    pub unk3: u32, // pad?
    pub unk4: u32, // pad?

    // TODO: pointed to by above?
    #[br(count = unk1)]
    pub samplers: Vec<Sampler>,
}

#[derive(BinRead, Debug)]
pub struct Sampler {
    #[br(map(|x: u32| x.into()))]
    pub flags: SamplerFlags,

    // Is this actually a float?
    pub unk2: f32,
}

/// Texture sampler settings for addressing and filtering.
#[bitsize(32)]
#[derive(DebugBits, FromBits, Clone, Copy)]
pub struct SamplerFlags {
    /// Sets wrap U to repeat when `true`.
    pub repeat_u: bool,
    /// Sets wrap V to repeat when `true`.
    pub repeat_v: bool,
    /// Sets wrap U to mirrored repeat when `true` regardless of repeat U.
    pub mirror_u: bool,
    /// Sets wrap V to mirrored repeat when `true` regardless of repeat V.
    pub mirror_v: bool,
    /// Sets min and mag filter to nearest when `true`.
    /// The min filter also depends on disable_mipmap_filter.
    pub nearest: bool,
    /// Sets all wrap modes to clamp and min and mag filter to linear.
    /// Ignores the values of previous flags.
    pub force_clamp: bool,
    /// Removes the mipmap nearest from the min filter when `true`.
    pub disable_mipmap_filter: bool,
    pub unk1: bool,
    pub unk3: bool,
    pub unk: u23,
}

/// A single material assignable to a [Mesh].
/// `ml::mdsMatInfoHeader` in the Xenoblade 2 binary.
#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct Material {
    #[br(parse_with = parse_string_ptr32, offset = base_offset)]
    pub name: String,

    #[br(map(|x: u32| x.into()))]
    pub flags: MaterialFlags,

    pub render_flags: u32,

    /// Color multiplier value assigned to the `gMatCol` shader uniform.
    pub color: [f32; 4],

    // TODO: final byte controls reference?
    pub alpha_test_ref: [u8; 4],

    // TODO: materials with zero textures?
    /// Defines the shader's sampler bindings in order for s0, s1, s2, ...
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub textures: Vec<Texture>,

    // TODO: rename to pipeline state?
    pub state_flags: StateFlags,

    // group indices?
    pub m_unks1_1: u32,
    pub m_unks1_2: u32,
    pub m_unks1_3: u32,
    pub m_unks1_4: u32,

    pub floats_start_index: u32, // work value index?

    // TODO: starts with a small number and then some random ints?
    pub ints_start_index: u32,
    pub ints_count: u32,

    // always count 1?
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub shader_programs: Vec<ShaderProgram>,

    pub unk5: u32,

    // index for MaterialUnk1.unk1?
    // work callbacks?
    pub unk_start_index: u16, // sum of previous unk_count?
    pub unk_count: u16,

    // TODO: alt textures offset for non opaque rendering?
    pub m_unks2: [u16; 3],

    /// Index into [alpha_test_textures](struct.Materials.html#structfield.alpha_test_textures).
    pub alpha_test_texture_index: u16,
    pub m_unks3: [u16; 8],
}

#[bitsize(32)]
#[derive(DebugBits, FromBits, Clone, Copy)]
pub struct MaterialFlags {
    pub unk1: bool,
    pub unk2: bool,
    /// Enables alpha testing from a texture when `true`.
    pub alpha_mask: bool,
    /// Samples `texture.x` from a dedicated mask texture when `true`.
    /// Otherwise, the alpha channel is used.
    pub separate_mask: bool,
    pub unk: u28,
}

#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateFlags {
    pub flag0: u8, // depth write?
    pub blend_state: BlendState,
    pub cull_mode: CullMode,
    pub flag3: u8, // unused?
    pub stencil_state1: StencilState1,
    pub stencil_state2: StencilState2,
    pub depth_func: DepthFunc,
    pub flag7: u8, // color writes?
}

// TODO: Convert these to equations for RGB and alpha for docs.
// TODO: Is it worth documenting this outside of xc3_wgpu?
// flag, col src, col dst, col op, alpha src, alpha dst, alpha op
// 0 = disabled
// 1, Src Alpha, 1 - Src Alpha, Add, Src Alpha, 1 - Src Alpha, Add
// 2, Src Alpha, One, Add, Src Alpha, One, Add
// 3, Zero, Src Col, Add, Zero, Src Col, Add
// 6, disabled + ???
#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[br(repr(u8))]
pub enum BlendState {
    Disabled = 0,
    AlphaBlend = 1,
    Additive = 2,
    Multiplicative = 3,
    Unk6 = 6, // also disabled?
}

// TODO: Get the actual stencil state from RenderDoc.
// 0 = disables hair blur stencil stuff?
// 4 = disables hair but different ref value?
// 16 = enables hair blur stencil stuff?
#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[br(repr(u8))]
pub enum StencilState1 {
    Always = 0,
    Unk1 = 1,
    Always2 = 4,
    Unk5 = 5,
    Unk8 = 8,
    Unk9 = 9,
    UnkHair = 16,
    Unk20 = 20,
}

// TODO: Does this flag actually disable stencil?
#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[br(repr(u8))]
pub enum StencilState2 {
    Disabled = 0,
    Enabled = 1,
    Unk2 = 2,
    Unk6 = 6,
    Unk7 = 7,
    Unk8 = 8,
}

#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[br(repr(u8))]
pub enum DepthFunc {
    Disabled = 0,
    LessEqual = 1,
    Equal = 3,
}

#[derive(BinRead, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[br(repr(u8))]
pub enum CullMode {
    Back = 0,
    Front = 1,
    Disabled = 2,
    Unk3 = 3, // front + ???
}

/// `ml::MdsMatMaterialTechnique` in the Xenoblade 2 binary.
#[derive(BinRead, Debug)]
pub struct ShaderProgram {
    /// Index into [shader_programs](struct.Materials.html#structfield.shader_programs).
    pub program_index: u32,
    pub unk_type: ShaderUnkType,
    pub parent_material_index: u16, // buffer index?
    pub flags: u32,                 // always 1?
}

// Affects what pass the object renders in?
// Each "pass" has different render targets?
// _trans = 1,
// _ope = 0,1,7
// _zpre = 0
// _outline = 0
#[derive(BinRead, Debug, PartialEq, Eq, Clone, Copy)]
#[br(repr(u16))]
pub enum ShaderUnkType {
    Unk0 = 0, // main opaque + some transparent?
    Unk1 = 1, // second layer transparent?
    Unk6 = 6, // used for maps?
    Unk7 = 7, // additional eye effect layer?
    Unk9 = 9, // used for maps?
}

#[derive(BinRead, Debug)]
pub struct Texture {
    pub texture_index: u16,
    pub sampler_index: u16,
    pub unk2: u16,
    pub unk3: u16,
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Models {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    pub unk1: u32,

    pub max_xyz: [f32; 3],
    pub min_xyz: [f32; 3],

    #[br(parse_with = parse_offset_count, args { offset: base_offset, inner: base_offset })]
    pub models: Vec<Model>,

    pub unk2: u32,

    #[br(parse_with = parse_opt_ptr32, offset = base_offset)]
    pub skeleton: Option<Skeleton>,

    pub unks3: [u32; 22],

    #[br(parse_with = parse_opt_ptr32, offset = base_offset)]
    pub unk_offset1: Option<MeshUnk1>,

    pub unk_offset2: u32,

    #[br(parse_with = parse_opt_ptr32, offset = base_offset)]
    pub lod_data: Option<LodData>,
    // TODO: more fields?
}

/// A collection of meshes where each [Mesh] represents one draw call.
///
/// Each [Model] has an associated [VertexData](crate::vertex::VertexData) containing vertex and index buffers.
#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct Model {
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub meshes: Vec<Mesh>,

    pub unk1: u32,
    pub max_xyz: [f32; 3],
    pub min_xyz: [f32; 3],
    pub bounding_radius: f32,
    pub unks: [u32; 7],
}

#[derive(BinRead, Debug)]
pub struct Mesh {
    pub render_flags: u32,
    pub skin_flags: u32,
    pub vertex_buffer_index: u16,
    pub index_buffer_index: u16,
    pub unk_index: u16,
    pub material_index: u16,
    pub unk2: u32,
    pub unk3: u32,
    pub unk4: u32,
    pub unk5: u16,
    pub lod: u16, // TODO: flags?
    // TODO: groups?
    pub unks6: [i32; 4],
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct MeshUnk1 {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_ptr32)]
    #[br(args { offset: base_offset, inner: base_offset })]
    pub inner: MeshUnk1Inner,
    pub unk1: [u32; 14],
}

#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct MeshUnk1Inner {
    #[br(parse_with = parse_string_ptr32, offset = base_offset)]
    pub unk1: String,

    pub unk2: [f32; 9],
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct LodData {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    pub unk1: u32,

    // TODO: Count related to number of mesh lod values?
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub items1: Vec<LodItem1>,

    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub items2: Vec<LodItem2>,

    pub unks: [u32; 4],
}

// TODO: is lod: 0 in the mxmd special?
#[derive(BinRead, Debug)]
pub struct LodItem1 {
    pub unk1: [u32; 4],
    pub unk2: f32,
    // second element is index related to count in item2?
    // [0,0,1,0], [0,1,1,0], [0,2,1,0], ...
    pub unk3: [u8; 4],
    pub unk4: [u32; 2],
}

// TODO: lod group?
#[derive(BinRead, Debug)]
pub struct LodItem2 {
    // TODO: base_lod_index?
    // TODO: (start_index, count) for items1?
    pub base_lod_index: u16,
    pub lod_count: u16,
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Textures {
    // TODO: The fields change depending on some sort of flag?
    pub tag: u32, // 4097 or sometimes 0?

    #[br(args_raw(tag))]
    pub inner: TexturesInner,
}

#[derive(BinRead, Debug)]
#[br(import_raw(tag: u32))]
pub enum TexturesInner {
    #[br(pre_assert(tag == 0))]
    Unk0(Textures1),
    #[br(pre_assert(tag == 4097))]
    Unk1(Textures2),
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Textures1 {
    // Subtract the tag size.
    #[br(temp, try_calc = r.stream_position().map(|p| p - 4))]
    base_offset: u64,

    pub unk1: u32, // TODO: count for multiple packed textures?
    // low textures?
    #[br(parse_with = parse_ptr32, offset = base_offset)]
    pub textures1: PackedExternalTextures,
    // high textures?
    #[br(parse_with = parse_opt_ptr32, offset = base_offset)]
    pub textures2: Option<PackedExternalTextures>,

    pub unk4: u32,
    pub unk5: u32,
    // TODO: more fields?
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Textures2 {
    // Subtract the tag size.
    #[br(temp, try_calc = r.stream_position().map(|p| p - 4))]
    base_offset: u64,

    pub unk2: u32, // 103

    // TODO: count offset?
    pub unk3: u32,
    pub unk4: u32,

    // TODO: count?
    pub unk5: u32,

    #[br(parse_with = parse_ptr32, offset = base_offset)]
    pub unk_offset: TexturesUnk,

    pub unks2: [u32; 7],

    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub indices: Vec<u16>,

    // TODO: separate PackedTextures and PackedExternalTextures?
    #[br(parse_with = parse_opt_ptr32, offset = base_offset)]
    pub textures: Option<PackedExternalTextures>,

    pub unk7: u32,

    // TODO: same as the type in msrd?
    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub resources: Vec<TextureResource>,
}

#[derive(BinRead, Debug)]
pub struct TexturesUnk {
    pub unk1: u32,
    pub unk2: u32,
    pub unk3: u32,
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct PackedTextures {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_count_offset, args { offset: base_offset, inner: base_offset })]
    pub textures: Vec<PackedTexture>,

    pub unk2: u32,
    pub strings_offset: u32,
}

#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct PackedTexture {
    pub unk1: u32,

    // TODO: Optimized function for reading bytes?
    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub mibl_data: Vec<u8>,

    #[br(parse_with = parse_string_ptr32, offset = base_offset)]
    pub name: String,
}

// TODO: The alignment here is only 2?
#[binread]
#[derive(Debug, Xc3Write)]
#[br(stream = r)]
#[xc3(base_offset)]
pub struct PackedExternalTextures {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_count_offset, args { offset: base_offset, inner: base_offset })]
    #[xc3(count_offset)]
    pub textures: Vec<PackedExternalTexture>,

    pub unk2: u32,
    pub strings_offset: u32,
}

#[derive(BinRead, Xc3Write, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct PackedExternalTexture {
    pub unk1: u32,

    // TODO: These offsets are for different places for maps and characters?
    pub mibl_length: u32,
    pub mibl_offset: u32,

    #[br(parse_with = parse_string_ptr32, offset = base_offset)]
    #[xc3(offset)]
    pub name: String,
}

#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Skeleton {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    pub count1: u32,
    pub count2: u32,

    // TODO: Find a simpler way of writing this?
    // TODO: helper for separate count.
    #[br(parse_with = parse_ptr32)]
    #[br(args {
        offset: base_offset,
        inner: args! {
            count: count1 as usize,
            inner: base_offset
        }
    })]
    pub bones: Vec<Bone>,

    /// Column-major transformation matrices for each of the bones in [bones](#structfield.bones).
    #[br(parse_with = parse_ptr32)]
    #[br(args { offset: base_offset, inner: args! { count: count1 as usize } })]
    pub transforms: Vec<[[f32; 4]; 4]>,

    pub unk_offset1: u32,
    pub unk_offset2: u32,

    // TODO: 0..count-1?
    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub unk3: Vec<u16>,

    #[br(parse_with = parse_ptr32)]
    #[br(args { offset: base_offset, inner: base_offset })]
    pub unk_offset4: SkeletonUnk4,
    pub unk_offset5: u32,

    // TODO: Disabled by something above for XC2?
    #[br(parse_with = parse_opt_ptr32, args { offset: base_offset, inner: base_offset })]
    #[br(if(!unk3.is_empty()))]
    pub as_bone_data: Option<AsBoneData>,
}

#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct SkeletonUnk4 {
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub unk1: Vec<()>, // TODO: type?
    pub unk_offset: u32,
    // TODO: padding?
}

// TODO: Data for AS_ bones?
#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct AsBoneData {
    #[br(parse_with = parse_offset_count, offset = base_offset)]
    pub bones: Vec<AsBone>,
    // TODO: more fields
}

#[derive(BinRead, Debug)]
pub struct AsBone {
    /// The index in [bones](struct.Skeleton.html#structfield.bones).
    pub bone_index: u16,
    /// The index in [bones](struct.Skeleton.html#structfield.bones) of the parent bone.
    pub parent_index: u16,
    pub unk: [u32; 19],
}

#[derive(BinRead, Debug)]
#[br(import_raw(base_offset: u64))]
pub struct Bone {
    #[br(parse_with = parse_string_ptr32, offset = base_offset)]
    pub name: String,
    pub unk1: f32,
    pub unk_type: u32,
    #[br(pad_after = 8)]
    pub unk_index: u32,
}

// TODO: pointer to decl_gbl_cac in ch001011011.wimdo?
#[binread]
#[derive(Debug)]
#[br(stream = r)]
pub struct Unk1 {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub unk1: Vec<Unk1Unk1>,

    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub unk2: Vec<Unk1Unk2>,

    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub unk3: Vec<Unk1Unk3>,

    // angle values?
    #[br(parse_with = parse_count_offset, offset = base_offset)]
    pub unk4: Vec<Unk1Unk4>,
}

#[derive(BinRead, Debug)]
pub struct Unk1Unk1 {
    pub index: u16,
    pub unk2: u16, // 1
}

#[derive(BinRead, Debug)]
pub struct Unk1Unk2 {
    pub unk1: u16, // 0
    pub index: u16,
    pub unk3: u16,
    pub unk4: u16,
    pub unk5: u32, // 0
}

#[derive(BinRead, Debug)]
pub struct Unk1Unk3 {
    pub unk1: u16,
    pub unk2: u16,
    pub unk3: u32,
    pub unk4: u16,
    pub unk5: u16,
    pub unk6: u16,
    pub unk7: u16,
}

#[derive(BinRead, Debug)]
pub struct Unk1Unk4 {
    pub unk1: f32,
    pub unk2: f32,
    pub unk3: f32,
    pub unk4: u32,
}
