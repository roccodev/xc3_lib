use std::collections::BTreeMap;

use crate::gltf::texture::{
    albedo_generated_key, metallic_roughness_generated_key, normal_generated_key, TextureCache,
};
use crate::{AddressMode, ImageTexture, MapRoot, ModelRoot, Sampler};
use gltf::json::validation::Checked::Valid;

use super::texture::{GeneratedImageKey, ImageIndex};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct MaterialKey {
    pub root_index: usize,
    pub group_index: usize,
    pub models_index: usize,
    pub material_index: usize,
}

pub fn create_materials(
    roots: &[ModelRoot],
    texture_cache: &mut TextureCache,
) -> (
    Vec<gltf::json::Material>,
    BTreeMap<MaterialKey, usize>,
    Vec<gltf::json::Texture>,
    Vec<gltf::json::texture::Sampler>,
) {
    let mut materials = Vec::new();
    let mut material_indices = BTreeMap::new();
    let mut textures = Vec::new();
    let mut samplers = Vec::new();

    for (root_index, root) in roots.iter().enumerate() {
        add_models(
            &root.models,
            &mut samplers,
            texture_cache,
            &mut textures,
            &mut materials,
            &mut material_indices,
            &root.image_textures,
            root_index,
            0,
            0,
        );
    }

    // TODO: proper sampler support for camdo?
    if samplers.is_empty() {
        samplers.push(gltf_json::texture::Sampler::default());
    }

    (materials, material_indices, textures, samplers)
}

pub fn create_map_materials(
    roots: &[MapRoot],
    texture_cache: &mut TextureCache,
) -> (
    Vec<gltf::json::Material>,
    BTreeMap<MaterialKey, usize>,
    Vec<gltf::json::Texture>,
    Vec<gltf::json::texture::Sampler>,
) {
    let mut materials = Vec::new();
    let mut material_indices = BTreeMap::new();
    let mut textures = Vec::new();
    let mut samplers = Vec::new();

    for (root_index, root) in roots.iter().enumerate() {
        for (group_index, group) in root.groups.iter().enumerate() {
            for (models_index, models) in group.models.iter().enumerate() {
                add_models(
                    models,
                    &mut samplers,
                    texture_cache,
                    &mut textures,
                    &mut materials,
                    &mut material_indices,
                    &root.image_textures,
                    root_index,
                    group_index,
                    models_index,
                );
            }
        }
    }

    // TODO: proper sampler support for camdo?
    if samplers.is_empty() {
        samplers.push(gltf_json::texture::Sampler::default());
    }

    (materials, material_indices, textures, samplers)
}

fn add_models(
    models: &crate::Models,
    samplers: &mut Vec<gltf_json::texture::Sampler>,
    texture_cache: &mut TextureCache,
    textures: &mut Vec<gltf_json::Texture>,
    materials: &mut Vec<gltf_json::Material>,
    material_indices: &mut BTreeMap<MaterialKey, usize>,
    image_textures: &[ImageTexture],
    root_index: usize,
    group_index: usize,
    models_index: usize,
) {
    // Each Models has its own separately indexed samplers.
    let sampler_base_index = samplers.len();
    samplers.extend(models.samplers.iter().map(create_sampler));

    for (material_index, material) in models.materials.iter().enumerate() {
        let material = create_material(
            material,
            texture_cache,
            textures,
            root_index,
            sampler_base_index,
            image_textures,
        );
        let material_flattened_index = materials.len();
        materials.push(material);

        material_indices.insert(
            MaterialKey {
                root_index,
                group_index,
                models_index,
                material_index,
            },
            material_flattened_index,
        );
    }
}

fn create_sampler(sampler: &Sampler) -> gltf::json::texture::Sampler {
    gltf::json::texture::Sampler {
        mag_filter: match sampler.mag_filter {
            crate::FilterMode::Nearest => Some(Valid(gltf::json::texture::MagFilter::Nearest)),
            crate::FilterMode::Linear => Some(Valid(gltf::json::texture::MagFilter::Linear)),
        },
        min_filter: match sampler.mag_filter {
            crate::FilterMode::Nearest => Some(Valid(gltf::json::texture::MinFilter::Nearest)),
            crate::FilterMode::Linear => Some(Valid(gltf::json::texture::MinFilter::Linear)),
        },
        wrap_s: Valid(wrapping_mode(sampler.address_mode_u)),
        wrap_t: Valid(wrapping_mode(sampler.address_mode_v)),
        ..Default::default()
    }
}

fn wrapping_mode(address_mode: AddressMode) -> gltf::json::texture::WrappingMode {
    match address_mode {
        AddressMode::ClampToEdge => gltf::json::texture::WrappingMode::ClampToEdge,
        AddressMode::Repeat => gltf::json::texture::WrappingMode::Repeat,
        AddressMode::MirrorRepeat => gltf::json::texture::WrappingMode::MirroredRepeat,
    }
}

fn create_material(
    material: &crate::Material,
    texture_cache: &mut TextureCache,
    textures: &mut Vec<gltf::json::Texture>,
    root_index: usize,
    sampler_base_index: usize,
    image_textures: &[ImageTexture],
) -> gltf::json::Material {
    let assignments = material.output_assignments(image_textures);

    let albedo_key = albedo_generated_key(material, &assignments, root_index);
    let albedo_index = texture_cache.insert(albedo_key);

    let normal_key = normal_generated_key(material, &assignments, root_index);
    let normal_index = texture_cache.insert(normal_key);

    let metallic_roughness_key =
        metallic_roughness_generated_key(material, &assignments, root_index);

    let metallic_roughness_index = texture_cache.insert(metallic_roughness_key);

    gltf::json::Material {
        name: Some(material.name.clone()),
        pbr_metallic_roughness: gltf::json::material::PbrMetallicRoughness {
            base_color_texture: albedo_index.map(|i| {
                let texture_index = add_texture(textures, &albedo_key, i, sampler_base_index);

                // Assume all channels have the same UV attribute and scale.
                let scale = albedo_key.red_index.and_then(|i| i.texcoord_scale);

                gltf::json::texture::Info {
                    index: gltf::json::Index::new(texture_index),
                    tex_coord: 0,
                    extensions: texture_transform_ext(scale),
                    extras: Default::default(),
                }
            }),
            metallic_roughness_texture: metallic_roughness_index.map(|i| {
                let texture_index =
                    add_texture(textures, &metallic_roughness_key, i, sampler_base_index);

                // Assume all channels have the same UV attribute and scale.
                let scale = metallic_roughness_key
                    .red_index
                    .and_then(|i| i.texcoord_scale);

                gltf::json::texture::Info {
                    index: gltf::json::Index::new(texture_index),
                    tex_coord: 0,
                    extensions: texture_transform_ext(scale),
                    extras: Default::default(),
                }
            }),
            ..Default::default()
        },
        normal_texture: normal_index.map(|i| {
            let texture_index = add_texture(textures, &normal_key, i, sampler_base_index);

            // TODO: Scale normal maps?
            gltf::json::material::NormalTexture {
                index: gltf::json::Index::new(texture_index),
                scale: 1.0,
                tex_coord: 0,
                extensions: None,
                extras: Default::default(),
            }
        }),
        occlusion_texture: metallic_roughness_index.map(|i| {
            let texture_index =
                add_texture(textures, &metallic_roughness_key, i, sampler_base_index);

            // TODO: Occlusion map scale?
            gltf::json::material::OcclusionTexture {
                // Only the red channel is sampled for the occlusion texture.
                // We can reuse the metallic roughness texture red channel here.
                index: gltf::json::Index::new(texture_index),
                strength: gltf::json::material::StrengthFactor(1.0),
                tex_coord: 0,
                extensions: None,
                extras: Default::default(),
            }
        }),
        emissive_texture: None, // TODO: emission?
        alpha_mode: if material.alpha_test.is_some() {
            Valid(gltf::json::material::AlphaMode::Mask)
        } else {
            Valid(gltf::json::material::AlphaMode::Opaque)
        },
        alpha_cutoff: material
            .alpha_test
            .as_ref()
            .map(|a| gltf::json::material::AlphaCutoff(a.ref_value)),
        ..Default::default()
    }
}

fn texture_transform_ext(
    scale: Option<[ordered_float::OrderedFloat<f32>; 2]>,
) -> Option<gltf_json::extensions::texture::Info> {
    // TODO: Don't assume the first UV map?
    scale.map(|[u, v]| gltf::json::extensions::texture::Info {
        texture_transform: Some(gltf::json::extensions::texture::TextureTransform {
            offset: gltf::json::extensions::texture::TextureTransformOffset([0.0; 2]),
            rotation: gltf::json::extensions::texture::TextureTransformRotation(0.0),
            scale: gltf::json::extensions::texture::TextureTransformScale([u.0, v.0]),
            tex_coord: Some(0),
            extras: None,
        }),
    })
}

fn add_texture(
    textures: &mut Vec<gltf::json::Texture>,
    image_key: &GeneratedImageKey,
    image_index: u32,
    sampler_base_index: usize,
) -> u32 {
    // The channel packing means an image could theoretically require 4 samplers.
    // The samplers are unlikely to differ in practice, so just pick one.
    let sampler_index = image_key
        .red_index
        .map(|ImageIndex { sampler, .. }| sampler);

    let texture_index = textures.len() as u32;
    textures.push(gltf::json::Texture {
        name: None,
        sampler: sampler_index.map(|sampler_index| {
            gltf::json::Index::new((sampler_index + sampler_base_index) as u32)
        }),
        source: gltf::json::Index::new(image_index),
        extensions: None,
        extras: Default::default(),
    });
    texture_index
}
