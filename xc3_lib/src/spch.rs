use crate::{parse_count_offset2, parse_offset_count, parse_string_ptr32};
use binrw::{args, binread, helpers::count_with, FilePtr32};
use serde::Serialize;

/// .wishp, embedded in .wismt and .wimdo
#[binread]
#[derive(Debug, Serialize)]
#[br(magic(b"HCPS"))]
#[br(stream = r)]
pub struct Spch {
    // Subtract the magic size.
    #[br(temp, try_calc = r.stream_position().map(|p| p - 4))]
    base_offset: u64,

    version: u32,

    #[br(temp)]
    programs_offset: u32,
    #[br(temp)]
    programs_count: u32,

    // TODO: Related to string section?
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub unk4s: Vec<(u32, u32, u32)>,

    // TODO: Save these as Vec<u8> to make later processing easier?
    slct_base_offset: u32,
    slct_section_length: u32,

    /// Compiled shader binaries.
    /// Alternates between vertex and fragment shaders.
    // TODO: Optimized function for reading bytes?
    #[br(parse_with = parse_offset_count, args_raw(base_offset))]
    pub xv4_section: Vec<u8>,

    // data before the xV4 section
    // same count as xV4 but with magic 0x34127698?
    // each has length 2176 (referenced in shaders?)
    unk_section_offset: u32,
    unk_section_length: u32,

    // TODO: Does this actually need the program count?
    #[br(parse_with = FilePtr32::parse, offset = base_offset)]
    #[br(args { inner: args! { count: programs_count as usize } })]
    pub string_section: StringSection,

    #[br(pad_after = 16)]
    unk7: u32,
    // end of header?

    // TODO: Move this earlier?
    #[br(count = programs_count)]
    #[br(args {
        inner: args! {
            slct_base_offset: base_offset + slct_base_offset as u64,
            unk_base_offset: base_offset + unk_section_offset as u64,
        }
    })]
    #[br(seek_before = std::io::SeekFrom::Start(base_offset + programs_offset as u64))]
    pub shader_programs: Vec<ShaderProgram>,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import { count: usize })]
pub struct StringSection {
    #[br(parse_with = count_with(count, parse_string_ptr32))]
    pub program_names: Vec<String>,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import { slct_base_offset: u64, unk_base_offset: u64 })]
pub struct ShaderProgram {
    #[br(parse_with = FilePtr32::parse)]
    #[br(args { offset: slct_base_offset, inner: args! { unk_base_offset } })]
    pub slct: Slct,

    unk1: u32,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(magic(b"SLCT"))]
#[br(import { unk_base_offset: u64, })]
#[br(stream = r)]
pub struct Slct {
    // Subtract the magic size.
    #[br(temp, try_calc = r.stream_position().map(|p| p - 4))]
    base_offset: u64,

    unk1: u32,

    #[br(parse_with = parse_count_offset2, args_raw(base_offset))]
    unk_strings: Vec<UnkString>,

    #[br(parse_with = parse_count_offset2, args_raw(base_offset))]
    pub nvsds: Vec<NvsdMetadataOffset>,

    unk5_count: u32,
    unk5_offset: u32,

    unk_offset: u32,

    unk_offset1: u32,

    #[br(parse_with = FilePtr32::parse, offset = unk_base_offset)]
    unk_item: UnkItem,

    unk_offset2: u32,

    // relative to xv4 base offset
    pub xv4_offset: u32,
    // vertex + fragment size for all NVSDs
    xv4_total_size: u32,

    unks1: [u32; 4],
    // end of slct main header?
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
struct UnkString {
    unk1: u32,
    unk2: u32,
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    text: String,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct NvsdMetadataOffset {
    #[br(parse_with = FilePtr32::parse, offset = base_offset)]
    pub inner: NvsdMetadata,
    size: u32,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(stream = r)]
pub struct NvsdMetadata {
    #[br(temp, try_calc = r.stream_position())]
    base_offset: u64,

    pub unks2: [u32; 8],

    pub unk_count1: u16,
    // TODO: not always the same as above?
    pub unk_count2: u16,

    #[br(parse_with = FilePtr32::parse)]
    #[br(args {
        offset: base_offset,
        inner: args! { count: unk_count1 as usize, inner: args! { base_offset } }
    })]
    pub buffers1: Vec<UniformBuffer>,

    pub unk13: u32, // end of strings offset?

    pub unk_count3: u16,
    // TODO: not always the same as above?
    pub unk_count4: u16,

    #[br(parse_with = FilePtr32::parse)]
    #[br(args {
        offset: base_offset,
        inner: args! { count: unk_count3 as usize, inner: args! { base_offset } }
    })]
    pub buffers2: Vec<UniformBuffer>,

    pub unk15: u32, // offset?

    #[br(temp)]
    sampler_count: u16,
    // TODO: not always the same as above?
    pub unk_count6: u16,

    #[br(parse_with = FilePtr32::parse)]
    #[br(args {
        offset: base_offset,
        inner: args! { count: sampler_count as usize, inner: args! { base_offset } }
    })]
    pub samplers: Vec<Sampler>,

    pub unks2_1: [u32; 4],

    #[br(parse_with = parse_count_offset2, args_raw(base_offset))]
    pub attributes: Vec<InputAttribute>,

    #[br(parse_with = parse_count_offset2, args_raw(base_offset))]
    pub uniforms: Vec<Uniform>,

    pub unks3: [u32; 4],

    // TODO: Separate this from the metadata type?
    pub nvsd: Nvsd,
}

#[binread]
#[derive(Debug, Serialize)]
struct UnkItem {
    unk: [u32; 9],
}

// TODO: Create a more meaningful default?
#[binread]
#[derive(Debug, Serialize, Default)]
#[br(magic(b"NVSD"))]
pub struct Nvsd {
    version: u32,
    unk1: u32, // 0
    unk2: u32, // 0
    unk3: u32, // identical to vertex_xv4_size?
    unk4: u32, // 0
    unk5: u32, // identical to unk_size1?
    // end of nvsd?

    // TODO: this section isn't always present?
    unk6: u32, // 1
    /// The size of the vertex shader pointed to by the [Slct].
    pub vertex_xv4_size: u32,
    /// The size of the fragment shader pointed to by the [Slct].
    pub fragment_xv4_size: u32,
    // Corresponding unk entry size for the two shaders?
    unk_size1: u32, // 2176
    unk_size2: u32, // 2176

    // TODO: What controls this count?
    unks4: [u16; 8],
}

// TODO: Annotate uniforms and uniform buffers?
#[binread]
#[derive(Debug, Serialize)]
#[br(import { base_offset: u64 })]
pub struct UniformBuffer {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,
    pub uniform_count: u16,
    pub uniform_start_index: u16,
    pub unk3: u32, // ??? + handle * 2?
    pub unk4: u16,
    pub unk5: u16,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import { base_offset: u64 })]
pub struct Sampler {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,
    pub unk1: u32,
    pub unk2: u32, // handle = (unk2 - 256) * 2 + 8?
}

/// A `vec4` parameter in a [UniformBuffer].
#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct Uniform {
    /// The name used to refer to the uniform like `gMatCol`.
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,

    /// The offset into the parent buffer in bytes.
    /// Usually a multiple of 16 since buffers are declared as `vec4 data[0x1000];`.
    pub buffer_offset: u32,
}

#[binread]
#[derive(Debug, Serialize)]
#[br(import_raw(base_offset: u64))]
pub struct InputAttribute {
    #[br(parse_with = parse_string_ptr32, args(base_offset))]
    pub name: String,
    pub location: u32,
}
