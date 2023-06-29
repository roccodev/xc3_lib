use std::{
    collections::HashMap,
    io::{Cursor, Read, Seek},
    path::Path,
};

use glam::{vec4, Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use xc3_lib::{
    mibl::Mibl,
    msmd::{ChannelType, MapParts, Msmd, StreamEntry},
    msrd::Msrd,
    mxmd::Mxmd,
    vertex::VertexData,
};
use xc3_model::{
    texture::{merge_mibl, ImageTexture},
    vertex::AttributeData,
};

use crate::{
    material::{foliage_materials, materials, Material},
    pipeline::{ModelPipelineData, PipelineKey},
    shader,
    texture::create_texture,
};

// Organize the model data to ensure shared resources are created only once.
pub struct ModelGroup {
    pub models: Vec<Model>,
    materials: Vec<Material>,
    // Cache pipelines by their creation parameters.
    pipelines: HashMap<PipelineKey, wgpu::RenderPipeline>,
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    vertex_buffers: Vec<VertexBuffer>,
    index_buffers: Vec<IndexBuffer>,
    // Use a collection to support "instancing" for map props.
    pub instances: Vec<ModelInstance>,
}

pub struct ModelInstance {
    per_model: crate::shader::model::bind_groups::BindGroup3,
}

#[derive(Debug)]
pub struct Mesh {
    vertex_buffer_index: usize,
    index_buffer_index: usize,
    material_index: usize,
    // TODO: lod?
}

struct VertexBuffer {
    vertex_buffer: wgpu::Buffer,
}

struct IndexBuffer {
    index_buffer: wgpu::Buffer,
    vertex_index_count: u32,
}

impl ModelGroup {
    // TODO: How to handle other unk types?
    pub fn draw<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>, is_transparent: bool) {
        // TODO: Is this the best way to "instance" models?
        for model in &self.models {
            for instance in &model.instances {
                instance.per_model.set(render_pass);

                // Each "instance" repeats the same meshes with different transforms.
                for mesh in &model.meshes {
                    // TODO: How does LOD selection work in game?
                    let material = &self.materials[mesh.material_index];

                    // TODO: Why are there materials with no textures?
                    // TODO: Group these into passes with separate shaders for each pass?
                    // TODO: The main pass is shared with outline, ope, and zpre?
                    // TODO: How to handle transparency?
                    // TODO: Characters render as solid white?
                    if (is_transparent != material.pipeline_key.write_to_all_outputs)
                        && !material.name.ends_with("_outline")
                        && !material.name.ends_with("_ope")
                        && !material.name.ends_with("_zpre")
                    {
                        // TODO: How to make sure the pipeline outputs match the render pass?
                        let pipeline = &self.pipelines[&material.pipeline_key];
                        render_pass.set_pipeline(pipeline);

                        material.bind_group1.set(render_pass);
                        material.bind_group2.set(render_pass);

                        self.draw_mesh(model, mesh, render_pass);
                    }
                }
            }
        }
    }

    fn draw_mesh<'a>(
        &'a self,
        model: &'a Model,
        mesh: &Mesh,
        render_pass: &mut wgpu::RenderPass<'a>,
    ) {
        let vertex_data = &model.vertex_buffers[mesh.vertex_buffer_index];
        render_pass.set_vertex_buffer(0, vertex_data.vertex_buffer.slice(..));

        // TODO: Are all indices u16?
        // TODO: Why do maps not always refer to a valid index buffer?
        let index_data = &model.index_buffers[mesh.index_buffer_index];
        // let index_data = &self.index_buffers[mesh.index_buffer_index];
        render_pass.set_index_buffer(index_data.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        render_pass.draw_indexed(0..index_data.vertex_index_count, 0, 0..1);
    }
}

pub fn load_model(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    msrd: &Msrd,
    mxmd: &Mxmd,
    model_path: &str,
    shader_database: &xc3_shader::gbuffer_database::GBufferDatabase,
) -> ModelGroup {
    // Compile shaders only once to improve loading times.
    let pipeline_data = ModelPipelineData::new(device);

    // TODO: Avoid unwrap.
    let vertex_data = msrd.extract_vertex_data();

    // "chr/en/file.wismt" -> "chr/tex/nx/m"
    // TODO: Don't assume model_path is in the chr/ch or chr/en folders.
    let chr_folder = Path::new(model_path).parent().unwrap().parent().unwrap();
    let m_tex_folder = chr_folder.join("tex").join("nx").join("m");
    let h_tex_folder = chr_folder.join("tex").join("nx").join("h");

    let textures = load_textures(device, queue, msrd, mxmd, &m_tex_folder, &h_tex_folder);

    // TODO: Don't assume there is only one model?
    let model =
        xc3_model::Model::from_model(&mxmd.models.models[0], &vertex_data, vec![Mat4::IDENTITY]);

    let model_folder = model_folder(model_path);
    let spch = shader_database.files.get(&model_folder);

    let (materials, pipelines) = materials(
        device,
        queue,
        &pipeline_data,
        &mxmd.materials,
        &textures,
        spch,
    );

    ModelGroup {
        materials,
        pipelines,
        models: vec![create_model(device, &model)],
    }
}

// TODO: Move this to xc3_shader?
fn model_folder(model_path: &str) -> String {
    Path::new(model_path)
        .with_extension("")
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string()
}

// TODO: Separate module for this?
// TODO: Better way to pass the wismda file?
pub fn load_map<R: Read + Seek>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    msmd: &Msmd,
    wismda: &mut R,
    model_path: &str,
    shader_database: &xc3_shader::gbuffer_database::GBufferDatabase,
) -> Vec<ModelGroup> {
    let model_folder = model_folder(model_path);

    // Compile shaders only once to improve loading times.
    let pipeline_data = ModelPipelineData::new(device);

    let textures: Vec<_> = msmd
        .textures
        .iter()
        .map(|texture| {
            // Load high resolution textures.
            // TODO: Merging doesn't always work?
            let base_mip_level = texture.high.decompress(wismda);
            let mibl_m = texture.mid.extract(wismda);
            merge_mibl(base_mip_level, mibl_m)
        })
        .collect();

    // TODO: Better way to combine models?
    let mut combined_models = Vec::new();
    for (i, env_model) in msmd.env_models.iter().enumerate() {
        let model = load_env_model(
            device,
            queue,
            wismda,
            env_model,
            i,
            &model_folder,
            shader_database,
            &pipeline_data,
        );
        combined_models.push(model);
    }

    for foliage_model in &msmd.foliage_models {
        let model = load_foliage_model(device, queue, wismda, foliage_model, &pipeline_data);
        combined_models.push(model);
    }

    for (i, map_model) in msmd.map_models.iter().enumerate() {
        let model = load_map_model_group(
            device,
            queue,
            wismda,
            map_model,
            i,
            &msmd.map_vertex_data,
            &textures,
            &model_folder,
            shader_database,
            &pipeline_data,
        );
        combined_models.push(model);
    }

    for (i, prop_model) in msmd.prop_models.iter().enumerate() {
        let model = load_prop_model_group(
            device,
            queue,
            wismda,
            prop_model,
            i,
            &msmd.prop_vertex_data,
            &textures,
            msmd.parts.as_ref(),
            &model_folder,
            shader_database,
            &pipeline_data,
        );
        combined_models.push(model);
    }

    combined_models
}

fn load_prop_model_group<R: Read + Seek>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wismda: &mut R,
    prop_model: &xc3_lib::msmd::PropModel,
    model_index: usize,
    prop_vertex_data: &[StreamEntry<VertexData>],
    image_textures: &[ImageTexture],
    parts: Option<&MapParts>,
    model_folder: &str,
    shader_database: &xc3_shader::gbuffer_database::GBufferDatabase,
    pipeline_data: &ModelPipelineData,
) -> ModelGroup {
    let prop_model_data = prop_model.entry.extract(wismda);

    // Get the textures referenced by the materials in this model.
    let textures = load_map_textures(device, queue, &prop_model_data.textures, image_textures);

    let spch = shader_database
        .map_files
        .get(model_folder)
        .and_then(|map| map.prop_models.get(model_index));

    // TODO: cached textures?
    let (materials, pipelines) = materials(
        device,
        queue,
        pipeline_data,
        &prop_model_data.materials,
        &textures,
        spch,
    );

    // Load the base LOD model for each prop model.
    let mut models: Vec<_> = prop_model_data
        .lods
        .props
        .iter()
        .enumerate()
        .map(|(i, prop_lod)| {
            let base_lod_index = prop_lod.base_lod_index as usize;
            let vertex_data_index = prop_model_data.model_vertex_data_indices[base_lod_index];

            // TODO: Also cache vertex and index buffer creation?
            let vertex_data = prop_vertex_data[vertex_data_index as usize].extract(wismda);

            // Find all the instances referencing this prop.
            let instances = prop_model_data
                .lods
                .instances
                .iter()
                .filter(|instance| instance.prop_index as usize == i)
                .map(|instance| Mat4::from_cols_array_2d(&instance.transform))
                .collect();

            let model = xc3_model::Model::from_model(
                &prop_model_data.models.models[base_lod_index],
                &vertex_data,
                instances,
            );
            create_model(device, &model)
        })
        .collect();

    // TODO: Is this the correct way to handle animated props?
    // TODO: Document how this works in xc3_lib.
    // Add additional animated prop instances to the appropriate models.
    if let Some(parts) = parts {
        add_animated_part_instances(device, &mut models, &prop_model_data, parts);
    }

    ModelGroup {
        materials,
        pipelines,
        models,
    }
}

fn add_animated_part_instances(
    device: &wgpu::Device,
    models: &mut [Model],
    prop_model_data: &xc3_lib::map::PropModelData,
    parts: &MapParts,
) {
    let start = prop_model_data.lods.animated_parts_start_index as usize;
    let count = prop_model_data.lods.animated_parts_count as usize;

    for i in start..start + count {
        let instance = &parts.animated_instances[i];
        let animation = &parts.instance_animations[i];

        // Each instance has a base transform as well as animation data.
        let mut transform = Mat4::from_cols_array_2d(&instance.transform);

        // Get the first frame of the animation channels.
        let mut translation: Vec3 = animation.translation.into();

        // TODO: Do these add to or replace the base values?
        for channel in &animation.channels {
            match channel.channel_type {
                ChannelType::TranslationX => {
                    translation.x += channel
                        .keyframes
                        .get(0)
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::TranslationY => {
                    translation.y += channel
                        .keyframes
                        .get(0)
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::TranslationZ => {
                    translation.z += channel
                        .keyframes
                        .get(0)
                        .map(|f| f.value)
                        .unwrap_or_default()
                }
                ChannelType::RotationX => (),
                ChannelType::RotationY => (),
                ChannelType::RotationZ => (),
                ChannelType::ScaleX => (),
                ChannelType::ScaleY => (),
                ChannelType::ScaleZ => (),
            }
        }
        // TODO: transform order?
        transform = Mat4::from_translation(translation) * transform;

        let per_model = per_model_bind_group(device, transform);
        let model_instance = ModelInstance { per_model };

        models[instance.prop_index as usize]
            .instances
            .push(model_instance);
    }
}

fn load_map_model_group<R: Read + Seek>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wismda: &mut R,
    model: &xc3_lib::msmd::MapModel,
    model_index: usize,
    vertex_data: &[xc3_lib::msmd::StreamEntry<VertexData>],
    textures: &[ImageTexture],
    model_folder: &str,
    shader_database: &xc3_shader::gbuffer_database::GBufferDatabase,
    pipeline_data: &ModelPipelineData,
) -> ModelGroup {
    let model_data = model.entry.extract(wismda);

    // Get the textures referenced by the materials in this model.
    let textures = load_map_textures(device, queue, &model_data.textures, textures);

    let spch = shader_database
        .map_files
        .get(model_folder)
        .and_then(|map| map.map_models.get(model_index));

    let (materials, pipelines) = materials(
        device,
        queue,
        pipeline_data,
        &model_data.materials,
        &textures,
        spch,
    );

    let mut models = Vec::new();

    for group in model_data.groups.groups {
        let vertex_data_index = group.vertex_data_index as usize;
        let vertex_data = vertex_data[vertex_data_index].extract(wismda);

        // Each group has a base and low detail vertex data index.
        // Each model has an assigned vertex data index.
        // Find all the base detail models and meshes for each group.
        for (model, index) in model_data
            .models
            .models
            .iter()
            .zip(model_data.groups.model_vertex_data_indices.iter())
        {
            if *index as usize == vertex_data_index {
                let model = xc3_model::Model::from_model(model, &vertex_data, vec![Mat4::IDENTITY]);
                models.push(create_model(device, &model));
            }
        }
    }

    ModelGroup {
        materials,
        pipelines,
        models,
    }
}

fn load_env_model<R: Read + Seek>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wismda: &mut R,
    model: &xc3_lib::msmd::EnvModel,
    model_index: usize,
    model_folder: &str,
    shader_database: &xc3_shader::gbuffer_database::GBufferDatabase,
    pipeline_data: &ModelPipelineData,
) -> ModelGroup {
    let model_data = model.entry.extract(wismda);

    // Environment models embed their own textures instead of using the MSMD.
    let textures: Vec<_> = model_data
        .textures
        .textures
        .iter()
        .map(|texture| {
            let texture = Mibl::read(&mut Cursor::new(&texture.mibl_data))
                .unwrap()
                .try_into()
                .unwrap();
            create_texture(device, queue, &texture)
                .create_view(&wgpu::TextureViewDescriptor::default())
        })
        .collect();

    let spch = shader_database
        .map_files
        .get(model_folder)
        .and_then(|map| map.env_models.get(model_index));

    let (materials, pipelines) = materials(
        device,
        queue,
        pipeline_data,
        &model_data.materials,
        &textures,
        spch,
    );

    let models = model_data
        .models
        .models
        .iter()
        .map(|model| {
            // TODO: Avoid creating vertex buffers more than once?
            let model =
                xc3_model::Model::from_model(model, &model_data.vertex_data, vec![Mat4::IDENTITY]);
            create_model(device, &model)
        })
        .collect();

    ModelGroup {
        materials,
        pipelines,
        models,
    }
}

fn load_foliage_model<R: Read + Seek>(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    wismda: &mut R,
    model: &xc3_lib::msmd::FoliageModel,
    pipeline_data: &ModelPipelineData,
) -> ModelGroup {
    let model_data = model.entry.extract(wismda);

    // Foliage models embed their own textures instead of using the MSMD.
    let textures: Vec<_> = model_data
        .textures
        .textures
        .iter()
        .map(|texture| {
            let texture = Mibl::read(&mut Cursor::new(&texture.mibl_data))
                .unwrap()
                .try_into()
                .unwrap();
            create_texture(device, queue, &texture)
                .create_view(&wgpu::TextureViewDescriptor::default())
        })
        .collect();

    let (materials, pipelines) = foliage_materials(
        device,
        queue,
        pipeline_data,
        &model_data.materials,
        &textures,
    );

    // TODO: foliage models are instanced somehow for grass clumps?
    let models = model_data
        .models
        .models
        .iter()
        .map(|model| {
            // TODO: Avoid creating vertex buffers more than once?
            let model =
                xc3_model::Model::from_model(model, &model_data.vertex_data, vec![Mat4::IDENTITY]);
            create_model(device, &model)
        })
        .collect();

    ModelGroup {
        materials,
        pipelines,
        models,
    }
}

fn load_map_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    textures: &[xc3_lib::map::Texture],
    image_textures: &[ImageTexture],
) -> Vec<wgpu::TextureView> {
    textures
        .iter()
        .map(|item| {
            // TODO: Handle texture index being -1?
            let texture = &image_textures[item.texture_index.max(0) as usize];
            create_texture(device, queue, texture)
                .create_view(&wgpu::TextureViewDescriptor::default())
        })
        .collect()
}

fn model_index_buffers(device: &wgpu::Device, model: &xc3_model::Model) -> Vec<IndexBuffer> {
    model
        .index_buffers
        .iter()
        .map(|buffer| {
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("index buffer"),
                contents: bytemuck::cast_slice(&buffer.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            IndexBuffer {
                index_buffer,
                vertex_index_count: buffer.indices.len() as u32,
            }
        })
        .collect()
}

fn load_textures(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    msrd: &Msrd,
    mxmd: &Mxmd,
    m_tex_folder: &Path,
    h_tex_folder: &Path,
) -> Vec<wgpu::TextureView> {
    let textures = xc3_model::texture::load_textures(msrd, mxmd, m_tex_folder, h_tex_folder);
    textures
        .iter()
        .map(|texture| {
            create_texture(device, queue, texture)
                .create_view(&wgpu::TextureViewDescriptor::default())
        })
        .collect()
}

fn create_model(device: &wgpu::Device, model: &xc3_model::Model) -> Model {
    let vertex_buffers = model_vertex_buffers(device, model);
    let index_buffers = model_index_buffers(device, model);

    let meshes = model
        .meshes
        .iter()
        .map(|mesh| Mesh {
            vertex_buffer_index: mesh.vertex_buffer_index,
            index_buffer_index: mesh.index_buffer_index,
            material_index: mesh.material_index,
        })
        .collect();

    let instances = model
        .instances
        .iter()
        .map(|t| {
            let per_model = per_model_bind_group(device, *t);

            ModelInstance { per_model }
        })
        .collect();

    Model {
        vertex_buffers,
        index_buffers,
        meshes,
        instances,
    }
}

fn model_vertex_buffers(device: &wgpu::Device, model: &xc3_model::Model) -> Vec<VertexBuffer> {
    model
        .vertex_buffers
        .iter()
        .map(|buffer| {
            let vertex_count = buffer
                .attributes
                .first()
                .map(|a| a.len())
                .unwrap_or_default();

            let mut vertices = vec![
                shader::model::VertexInput {
                    position: Vec3::ZERO,
                    weight_index: 0,
                    vertex_color: Vec4::ZERO,
                    normal: Vec4::ZERO,
                    tangent: Vec4::ZERO,
                    uv1: Vec4::ZERO,
                };
                vertex_count
            ];

            // Convert the attributes back to an interleaved representation for rendering.
            // Unused attributes will use the default values defined above.
            // Using a single vertex representation reduces the number of shaders.
            set_attributes(&mut vertices, buffer);

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("vertex buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

            VertexBuffer { vertex_buffer }
        })
        .collect()
}

fn set_attributes(verts: &mut [shader::model::VertexInput], buffer: &xc3_model::VertexBuffer) {
    for attribute in &buffer.attributes {
        match attribute {
            AttributeData::Position(vals) => set_attribute(verts, vals, |v, t| v.position = t),
            AttributeData::Normal(vals) => set_attribute(verts, vals, |v, t| v.normal = t),
            AttributeData::Tangent(vals) => set_attribute(verts, vals, |v, t| v.tangent = t),
            AttributeData::Uv1(vals) => {
                set_attribute(verts, vals, |v, t| v.uv1 = vec4(t.x, t.y, 0.0, 0.0))
            }
            AttributeData::Uv2(_) => (),
            AttributeData::VertexColor(vals) => {
                set_attribute(verts, vals, |v, t| v.vertex_color = t)
            }
            AttributeData::WeightIndex(vals) => {
                set_attribute(verts, vals, |v, t| v.weight_index = t)
            }
        }
    }
}

fn set_attribute<T, F>(vertices: &mut [shader::model::VertexInput], values: &[T], assign: F)
where
    T: Copy,
    F: Fn(&mut shader::model::VertexInput, T),
{
    for (vertex, value) in vertices.iter_mut().zip(values) {
        assign(vertex, *value);
    }
}

fn per_model_bind_group(
    device: &wgpu::Device,
    transform: glam::Mat4,
) -> shader::model::bind_groups::BindGroup3 {
    let per_model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("per model buffer"),
        contents: bytemuck::cast_slice(&[crate::shader::model::PerModel { matrix: transform }]),
        usage: wgpu::BufferUsages::UNIFORM,
    });

    crate::shader::model::bind_groups::BindGroup3::from_bindings(
        device,
        crate::shader::model::bind_groups::BindGroupLayout3 {
            per_model: per_model_buffer.as_entire_buffer_binding(),
        },
    )
}
