use std::{io::Cursor, path::Path};

use glam::{Mat4, Vec3};
use indexmap::IndexMap;
use log::error;
use rayon::prelude::*;
use thiserror::Error;
use xc3_lib::{
    error::DecompressStreamError,
    map::{FoliageMaterials, PropInstance, PropLod, PropPositions},
    mibl::Mibl,
    msmd::{ChannelType, MapParts, Msmd, StreamEntry},
    mxmd::{RenderPassType, StateFlags, TextureUsage},
};

use crate::{
    create_materials, create_samplers, model_name,
    shader_database::ShaderDatabase,
    texture::{self, CreateImageTextureError, ImageTexture},
    Material, Model, ModelBuffers, ModelGroup, ModelRoot, Models, Texture,
};

#[derive(Debug, Error)]
pub enum LoadMapError {
    #[error("error reading data: {0}")]
    Io(#[from] std::io::Error),

    #[error("error reading data: {0}")]
    Binrw(#[from] binrw::Error),

    #[error("error loading image texture: {0}")]
    Image(#[from] texture::CreateImageTextureError),

    #[error("error decompressing stream: {0}")]
    Stream(#[from] xc3_lib::error::DecompressStreamError),
}

/// Load a map from a `.wismhd` file.
/// The corresponding `.wismda` should be in the same directory.
///
/// # Examples
/// ``` rust no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use xc3_model::{load_map, shader_database::ShaderDatabase};
///
/// let database = ShaderDatabase::from_file("xc1.json")?;
/// let roots = load_map("xeno1/map/ma000.wismhd", Some(&database))?;
///
/// let database = ShaderDatabase::from_file("xc2.json")?;
/// let roots = load_map("xeno2/map/ma01a.wismhd", Some(&database))?;
///
/// let database = ShaderDatabase::from_file("xc3.json")?;
/// let roots = load_map("xeno3/map/ma01a.wismhd", Some(&database))?;
/// # Ok(())
/// # }
/// ```
pub fn load_map<P: AsRef<Path>>(
    wismhd_path: P,
    shader_database: Option<&ShaderDatabase>,
) -> Result<Vec<ModelRoot>, LoadMapError> {
    let msmd = Msmd::from_file(wismhd_path.as_ref())?;
    let wismda = std::fs::read(wismhd_path.as_ref().with_extension("wismda"))?;

    // Loading is CPU intensive due to decompression and decoding.
    // The .wismda is loaded into memory as &[u8].
    // Extracting can be parallelized without locks by creating multiple readers.
    let model_folder = model_name(wismhd_path.as_ref());

    // Some maps don't use XBC1 compressed archives in the .wismda file.
    let compressed = msmd.wismda_info.compressed_length != msmd.wismda_info.decompressed_length;

    // TODO: Better way to combine models?
    let mut roots = Vec::new();

    for (i, model) in msmd.env_models.iter().enumerate() {
        let root = load_env_model(
            &wismda,
            compressed,
            model,
            i,
            &model_folder,
            shader_database,
        )?;
        roots.push(root);
    }

    for foliage_model in &msmd.foliage_models {
        let root = load_foliage_model(&wismda, compressed, foliage_model)?;
        roots.push(root);
    }

    // TODO: How much does a mutable cache negatively impact parallelization?
    // TODO: Is there enough reuse for it to be worth caching these?
    let mut texture_cache = TextureCache::new(&msmd, &wismda, compressed)?;

    let map_model_group = map_models_group(
        &msmd,
        &wismda,
        compressed,
        &model_folder,
        &mut texture_cache,
        shader_database,
    )?;

    let prop_model_group = props_group(
        &msmd,
        &wismda,
        compressed,
        model_folder,
        &mut texture_cache,
        shader_database,
    )?;

    roots.push(ModelRoot {
        groups: vec![map_model_group, prop_model_group],
        image_textures: texture_cache.image_textures()?,
        skeleton: None,
    });

    Ok(roots)
}

// TODO: Is there a better way of doing this?
// Lazy loading for the image textures.
struct TextureCache {
    low_textures: Vec<Vec<(TextureUsage, Mibl)>>,
    high_textures: Vec<Mibl>,
    // Use a map that preserves insertion order to get consistent ordering.
    texture_to_image_texture_index: IndexMap<(i16, i16, i16), usize>,
}

impl TextureCache {
    fn new(msmd: &Msmd, wismda: &[u8], compressed: bool) -> Result<Self, LoadMapError> {
        let low_textures = msmd
            .low_textures
            .par_iter()
            .map(|e| {
                let textures = e.extract(&mut Cursor::new(&wismda), compressed)?;
                textures
                    .textures
                    .iter()
                    .map(|t| Ok((t.usage, Mibl::from_bytes(&t.mibl_data)?)))
                    .collect::<Result<Vec<_>, LoadMapError>>()
            })
            .collect::<Result<Vec<_>, _>>()?;

        let high_textures = msmd
            .textures
            .par_iter()
            .map(|texture| {
                let mut wismda = Cursor::new(&wismda);
                let mibl_m = texture.mid.extract(&mut wismda, compressed)?;

                if texture.base_mip.decompressed_size > 0 {
                    let base_mip_level = texture.base_mip.decompress(&mut wismda, compressed)?;

                    Ok(mibl_m.with_base_mip(&base_mip_level))
                } else {
                    Ok(mibl_m)
                }
            })
            .collect::<Result<Vec<_>, LoadMapError>>()?;

        Ok(Self {
            texture_to_image_texture_index: IndexMap::new(),
            low_textures,
            high_textures,
        })
    }

    fn insert(&mut self, texture: &xc3_lib::map::Texture) -> usize {
        let key = (
            texture.low_texture_index,
            texture.low_textures_entry_index,
            texture.texture_index,
        );
        let new_index = self.texture_to_image_texture_index.len();
        *self
            .texture_to_image_texture_index
            .entry(key)
            .or_insert(new_index)
    }

    fn get_low_texture(&self, entry_index: i16, index: i16) -> Option<&(TextureUsage, Mibl)> {
        let entry_index = usize::try_from(entry_index).ok()?;
        let index = usize::try_from(index).ok()?;
        self.low_textures.get(entry_index)?.get(index)
    }

    fn get_high_texture(&self, index: i16) -> Option<&Mibl> {
        let index = usize::try_from(index).ok()?;
        self.high_textures.get(index)
    }

    fn image_textures(&self) -> Result<Vec<ImageTexture>, CreateImageTextureError> {
        self.texture_to_image_texture_index
            .par_iter()
            .map(
                |((low_texture_index, low_textures_entry_index, texture_index), _)| {
                    let low = self.get_low_texture(*low_textures_entry_index, *low_texture_index);

                    if let Some(mibl) = self
                        .get_high_texture(*texture_index)
                        .or(low.map(|low| &low.1))
                    {
                        ImageTexture::from_mibl(mibl, None, low.map(|l| l.0)).map_err(Into::into)
                    } else {
                        // TODO: What do do if both indices are negative?
                        error!("No mibl for low: {low_texture_index}, low entry: {low_textures_entry_index}, high: {texture_index}");
                        let (usage, mibl) = self.get_low_texture(0, 0).unwrap();
                        ImageTexture::from_mibl(mibl, None, Some(*usage)).map_err(Into::into)
                    }
                },
            )
            .collect()
    }
}

fn map_models_group(
    msmd: &Msmd,
    wismda: &Vec<u8>,
    compressed: bool,
    model_folder: &str,
    texture_cache: &mut TextureCache,
    shader_database: Option<&ShaderDatabase>,
) -> Result<ModelGroup, LoadMapError> {
    let buffers = create_buffers(&msmd.map_vertex_data, wismda, compressed)?;

    // Decompression is expensive, so run in parallel ahead of time.
    let map_model_data = msmd
        .map_models
        .par_iter()
        .map(|m| m.entry.extract(&mut Cursor::new(wismda), compressed))
        .collect::<Result<Vec<_>, _>>()?;

    let mut models = Vec::new();
    models.extend(map_model_data.iter().enumerate().map(|(i, model_data)| {
        // Remove one layer of indirection from texture lookups.
        let material_root_texture_indices: Vec<_> = model_data
            .textures
            .iter()
            .map(|t| texture_cache.insert(t))
            .collect();

        load_map_model_group(
            model_data,
            i,
            model_folder,
            &material_root_texture_indices,
            shader_database,
        )
    }));

    Ok(ModelGroup { models, buffers })
}

fn props_group(
    msmd: &Msmd,
    wismda: &Vec<u8>,
    compressed: bool,
    model_folder: String,
    texture_cache: &mut TextureCache,
    shader_database: Option<&ShaderDatabase>,
) -> Result<ModelGroup, LoadMapError> {
    let buffers = create_buffers(&msmd.prop_vertex_data, wismda, compressed)?;

    // Decompression is expensive, so run in parallel ahead of time.
    let prop_positions: Vec<_> = msmd
        .prop_positions
        .par_iter()
        .map(|p| p.extract(&mut Cursor::new(wismda), compressed))
        .collect::<Result<Vec<_>, _>>()?;

    let prop_model_data: Vec<_> = msmd
        .prop_models
        .par_iter()
        .map(|m| m.entry.extract(&mut Cursor::new(wismda), compressed))
        .collect::<Result<Vec<_>, _>>()?;

    let models = prop_model_data
        .iter()
        .enumerate()
        .map(|(i, model_data)| {
            // Remove one layer of indirection from texture lookups.
            let material_root_texture_indices: Vec<_> = model_data
                .textures
                .iter()
                .map(|t| texture_cache.insert(t))
                .collect();

            load_prop_model_group(
                model_data,
                i,
                msmd.parts.as_ref(),
                &prop_positions,
                &model_folder,
                &material_root_texture_indices,
                shader_database,
            )
        })
        .collect();

    Ok(ModelGroup { models, buffers })
}

fn create_buffers(
    vertex_data: &[StreamEntry<xc3_lib::vertex::VertexData>],
    wismda: &Vec<u8>,
    compressed: bool,
) -> Result<Vec<ModelBuffers>, DecompressStreamError> {
    // Process vertex data ahead of time in parallel.
    // This gives better CPU utilization and avoids redundant processing.
    vertex_data
        .par_iter()
        .map(|e| {
            // Assume maps have no skeletons for now.
            let vertex_data = e.extract(&mut Cursor::new(wismda), compressed)?;
            ModelBuffers::from_vertex_data(&vertex_data, None).map_err(Into::into)
        })
        .collect()
}

fn load_prop_model_group(
    model_data: &xc3_lib::map::PropModelData,
    model_index: usize,
    parts: Option<&MapParts>,
    prop_positions: &[PropPositions],
    model_folder: &str,
    material_root_texture_indices: &[usize],
    shader_database: Option<&ShaderDatabase>,
) -> Models {
    let spch = shader_database
        .and_then(|database| database.map_files.get(model_folder))
        .and_then(|map| map.prop_models.get(model_index));

    // Calculate instances separately from models.
    // This allows us to avoid loading unused models later.
    let mut model_instances = vec![Vec::new(); model_data.models.models.len()];

    // Load instances for each base LOD model.
    add_prop_instances(
        &mut model_instances,
        &model_data.lods.props,
        &model_data.lods.instances,
    );

    // Add additional instances if present.
    for info in &model_data.prop_info {
        let additional_instances = &prop_positions[info.prop_position_entry_index as usize];

        add_prop_instances(
            &mut model_instances,
            &model_data.lods.props,
            &additional_instances.instances,
        );

        // TODO: Add animated parts from the additional instances
        // TODO: This doesn't work on all maps?
    }

    // TODO: Is this the correct way to handle animated props?
    // TODO: Document how this works in xc3_lib.
    // Add additional animated prop instances to the appropriate models.
    if let Some(parts) = parts {
        add_animated_part_instances(
            &mut model_instances,
            model_data.lods.animated_parts_start_index as usize,
            model_data.lods.animated_parts_count as usize,
            parts,
        );
    }

    // TODO: Group by vertex data index?
    // TODO: empty groups?

    let mut materials = create_materials(&model_data.materials, spch);
    apply_material_texture_indices(&mut materials, material_root_texture_indices);

    let samplers = create_samplers(&model_data.materials);

    let mut models = Models {
        models: Vec::new(),
        materials,
        samplers,
        base_lod_indices: model_data
            .models
            .lod_data
            .as_ref()
            .map(|data| data.groups.iter().map(|i| i.base_lod_index).collect()),
        min_xyz: model_data.models.min_xyz,
        max_xyz: model_data.models.max_xyz,
    };

    for ((model, vertex_data_index), instances) in model_data
        .models
        .models
        .iter()
        .zip(model_data.model_vertex_data_indices.iter())
        .zip(model_instances.into_iter())
    {
        // Avoid loading unused prop models.
        if !instances.is_empty() {
            let group = Model::from_model(model, instances, *vertex_data_index as usize);
            models.models.push(group);
        }
    }

    models
}

fn add_prop_instances(
    model_instances: &mut [Vec<Mat4>],
    props: &[PropLod],
    instances: &[PropInstance],
) {
    // TODO: Why do XC2 maps have instances for empty models?
    if !model_instances.is_empty() {
        for instance in instances {
            let prop_lod = &props[instance.prop_index as usize];
            // Only the first 28 bits should be used to properly load XC3 DLC maps.
            let base_lod_index = (prop_lod.base_lod_index & 0xFFFFFFF) as usize;
            // TODO: Should we also index into the PropModelLod?
            // TODO: Is PropModelLod.index always the same as its index in the list?
            model_instances[base_lod_index].push(Mat4::from_cols_array_2d(&instance.transform));
        }
    }
}

fn add_animated_part_instances(
    model_instances: &mut [Vec<Mat4>],
    start_index: usize,
    count: usize,
    parts: &MapParts,
) {
    for i in start_index..start_index + count {
        let instance = &parts.animated_instances[i];
        let animation = &parts.instance_animations[i];

        // Each instance has a base transform as well as animation data.
        let mut transform = Mat4::from_cols_array_2d(&instance.transform);

        // Get the first frame of the animation channels.
        let mut translation: Vec3 = animation.translation.into();

        let mut scale = Vec3::ONE;

        let mut rot_x = 0.0;
        let mut rot_y = 0.0;
        let mut rot_z = 0.0;

        // TODO: Do these add to or replace the base values?
        for channel in &animation.channels {
            match channel.channel_type {
                ChannelType::TranslationX => {
                    translation.x += channel
                        .keyframes
                        .first()
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::TranslationY => {
                    translation.y += channel
                        .keyframes
                        .first()
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::TranslationZ => {
                    translation.z += channel
                        .keyframes
                        .first()
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::RotationX => {
                    rot_x = channel
                        .keyframes
                        .first()
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::RotationY => {
                    rot_y = channel
                        .keyframes
                        .first()
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::RotationZ => {
                    rot_z = channel
                        .keyframes
                        .first()
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::ScaleX => {
                    scale.x = channel.keyframes.first().map(|f| f.value).unwrap_or(1.0)
                }
                ChannelType::ScaleY => {
                    scale.y = channel.keyframes.first().map(|f| f.value).unwrap_or(1.0)
                }
                ChannelType::ScaleZ => {
                    scale.z = channel.keyframes.first().map(|f| f.value).unwrap_or(1.0)
                }
            }
        }
        // TODO: transform order?
        transform = Mat4::from_translation(translation)
            * Mat4::from_euler(glam::EulerRot::XYZ, rot_x, rot_y, rot_z)
            * Mat4::from_scale(scale)
            * transform;
        model_instances[instance.prop_index as usize].push(transform);
    }
}

fn load_map_model_group(
    model_data: &xc3_lib::map::MapModelData,
    model_index: usize,
    model_folder: &str,
    material_root_texture_indices: &[usize],
    shader_database: Option<&ShaderDatabase>,
) -> Models {
    let spch = shader_database
        .and_then(|database| database.map_files.get(model_folder))
        .and_then(|map| map.map_models.get(model_index));

    let mut materials = create_materials(&model_data.materials, spch);
    apply_material_texture_indices(&mut materials, material_root_texture_indices);

    let samplers = create_samplers(&model_data.materials);

    // Each group has a base and low detail vertex data index.
    // Each model has an assigned vertex data index.
    // Find all the base detail models for each group.
    let models = model_data
        .groups
        .model_group_index
        .iter()
        .zip(model_data.models.models.iter())
        .filter_map(|(group_index, model)| {
            // TODO: Will filtering like this correctly select only the base LOD?
            model_data
                .groups
                .groups
                .get(*group_index as usize)
                .map(|group| {
                    let vertex_data_index = group.vertex_data_index as usize;
                    Model::from_model(model, vec![Mat4::IDENTITY], vertex_data_index)
                })
        })
        .collect();

    Models {
        models,
        materials,
        samplers,
        base_lod_indices: model_data
            .models
            .lod_data
            .as_ref()
            .map(|data| data.groups.iter().map(|i| i.base_lod_index).collect()),
        min_xyz: model_data.models.min_xyz,
        max_xyz: model_data.models.max_xyz,
    }
}

fn load_env_model(
    wismda: &[u8],
    compressed: bool,
    model: &xc3_lib::msmd::EnvModel,
    model_index: usize,
    model_folder: &str,
    shader_database: Option<&ShaderDatabase>,
) -> Result<ModelRoot, LoadMapError> {
    let mut wismda = Cursor::new(&wismda);

    let model_data = model.entry.extract(&mut wismda, compressed)?;

    // Environment models embed their own textures instead of using the MSMD.
    let image_textures = model_data
        .textures
        .textures
        .iter()
        .map(ImageTexture::from_packed_texture)
        .collect::<Result<Vec<_>, _>>()?;

    let spch = shader_database
        .and_then(|database| database.map_files.get(model_folder))
        .and_then(|map| map.env_models.get(model_index));

    let buffers = ModelBuffers::from_vertex_data(&model_data.vertex_data, None)?;

    Ok(ModelRoot {
        groups: vec![ModelGroup {
            models: vec![Models::from_models(
                &model_data.models,
                &model_data.materials,
                spch,
            )],
            buffers: vec![buffers],
        }],
        image_textures,
        skeleton: None,
    })
}

fn load_foliage_model(
    wismda: &[u8],
    compressed: bool,
    model: &xc3_lib::msmd::FoliageModel,
) -> Result<ModelRoot, LoadMapError> {
    let mut wismda = Cursor::new(&wismda);

    let model_data = model.entry.extract(&mut wismda, compressed)?;

    // Foliage models embed their own textures instead of using the MSMD.
    let image_textures = model_data
        .textures
        .textures
        .iter()
        .map(ImageTexture::from_packed_texture)
        .collect::<Result<Vec<_>, _>>()?;

    let materials = foliage_materials(&model_data.materials);

    // TODO: foliage models are instanced somehow for grass clumps?
    let models = model_data
        .models
        .models
        .iter()
        .map(|model| Model::from_model(model, vec![Mat4::IDENTITY], 0))
        .collect();

    let buffers = ModelBuffers::from_vertex_data(&model_data.vertex_data, None)?;

    // TODO: foliage samplers?
    // TODO: is it worth making a skeleton here?
    Ok(ModelRoot {
        groups: vec![ModelGroup {
            models: vec![Models {
                models,
                materials,
                samplers: Vec::new(),
                base_lod_indices: model_data
                    .models
                    .lod_data
                    .map(|data| data.groups.iter().map(|i| i.base_lod_index).collect()),
                min_xyz: model_data.models.min_xyz,
                max_xyz: model_data.models.max_xyz,
            }],
            buffers: vec![buffers],
        }],
        image_textures,
        skeleton: None,
    })
}

fn foliage_materials(materials: &FoliageMaterials) -> Vec<Material> {
    let materials = materials
        .materials
        .iter()
        .map(|material| {
            // TODO: Where are the textures?
            let textures = vec![Texture {
                image_texture_index: 0,
                sampler_index: 0,
            }];

            // TODO: Foliage shaders?
            let shader = None;

            // TODO: Flags?
            let flags = StateFlags {
                flag0: 0,
                blend_state: xc3_lib::mxmd::BlendState::Disabled,
                cull_mode: xc3_lib::mxmd::CullMode::Disabled,
                flag3: 0,
                stencil_state1: xc3_lib::mxmd::StencilState1::Always,
                stencil_state2: xc3_lib::mxmd::StencilState2::Disabled,
                depth_func: xc3_lib::mxmd::DepthFunc::LessEqual,
                flag7: 0,
            };

            Material {
                name: material.name.clone(),
                flags,
                textures,
                alpha_test: None,
                shader,
                unk_type: RenderPassType::Unk0,
                parameters: Default::default(),
            }
        })
        .collect();

    materials
}

fn apply_material_texture_indices(
    materials: &mut Vec<Material>,
    material_root_texture_indices: &[usize],
) {
    // Maps use material textures -> model data textures -> msmd textures.
    // Not all textures are referenced by each material.
    // xc3_model uses material textures -> root textures.
    // Apply indices here to reduce indirection for consuming code.
    for material in materials {
        for texture in &mut material.textures {
            let index = material_root_texture_indices[texture.image_texture_index];
            texture.image_texture_index = index;
        }
    }
}
