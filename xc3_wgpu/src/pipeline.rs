use xc3_lib::mxmd::{BlendState, MaterialFlags};

use crate::{DEPTH_FORMAT, GBUFFER_COLOR_FORMAT};

// TODO: Always set depth and stencil state?
pub fn model_pipeline(device: &wgpu::Device) -> wgpu::RenderPipeline {
    let module = crate::shader::model::create_shader_module(device);
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Model Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: crate::shader::model::vertex_state(
            &module,
            &crate::shader::model::vs_main_entry(wgpu::VertexStepMode::Vertex),
        ),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: crate::shader::model::ENTRY_FS_MAIN,
            // TODO: Get output count from wgsl_to_wgpu?
            targets: &vec![
                Some(wgpu::ColorTargetState {
                    format: GBUFFER_COLOR_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::all(),
                });
                7
            ],
        }),
        primitive: wgpu::PrimitiveState {
            // TODO: Do all meshes using indexed triangle lists?
            topology: wgpu::PrimitiveTopology::TriangleList,
            polygon_mode: wgpu::PolygonMode::Fill,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

pub fn model_transparent_pipeline(
    device: &wgpu::Device,
    flags: &MaterialFlags,
) -> wgpu::RenderPipeline {
    let module = crate::shader::model::create_shader_module(device);
    let render_pipeline_layout = crate::shader::model::create_pipeline_layout(device);

    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Model Transparent Pipeline"),
        layout: Some(&render_pipeline_layout),
        vertex: crate::shader::model::vertex_state(
            &module,
            &crate::shader::model::vs_main_entry(wgpu::VertexStepMode::Vertex),
        ),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: crate::shader::model::ENTRY_FS_TRANSPARENT,
            // TODO: alpha blending?
            // Create a target for each of the G-Buffer textures.
            // TODO: check outputs in wgsl_to_wgpu?
            targets: &vec![Some(wgpu::ColorTargetState {
                format: GBUFFER_COLOR_FORMAT,
                blend: blend_state(flags.blend_state),
                write_mask: wgpu::ColorWrites::all(),
            })],
        }),
        primitive: wgpu::PrimitiveState {
            // TODO: Do all meshes using indexed triangle lists?
            topology: wgpu::PrimitiveTopology::TriangleList,
            polygon_mode: wgpu::PolygonMode::Fill,
            cull_mode: Some(wgpu::Face::Back),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: DEPTH_FORMAT,
            depth_write_enabled: true,
            depth_compare: wgpu::CompareFunction::LessEqual,
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

fn blend_state(state: BlendState) -> Option<wgpu::BlendState> {
    match state {
        BlendState::Disabled => None,
        BlendState::AlphaBlend => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendState::Additive => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendState::Multiplicative => Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::Src,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::Src,
                operation: wgpu::BlendOperation::Add,
            },
        }),
        BlendState::Unk6 => None,
    }
}
