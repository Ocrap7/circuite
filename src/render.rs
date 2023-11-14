use egui::{load::SizedTexture, Frame};
use vello::{
    block_on_wgpu,
    kurbo::{Affine, Rect},
    peniko::{Brush, Color, Fill},
    util::{RenderContext, RenderSurface},
    RenderParams, Renderer, RendererOptions, Scene, SceneBuilder,
};
use winit::window::Window;

use crate::element::ElementManager;

pub struct UiState {
    viewport_tex: egui::TextureId,
}

impl UiState {
    pub fn new(viewport: egui::TextureId) -> UiState {
        UiState {
            viewport_tex: viewport,
        }
    }
}

pub struct RenderManager {
    pub ctx: RenderContext,
    pub main_surface: RenderSurface,

    vello_surface: wgpu::Texture,

    converter: TextureConverter,
    convert_surface: wgpu::Texture,

    renderer: Renderer,

    gui_renderer: egui_wgpu::Renderer,
    gui_output: Vec<egui::ClippedPrimitive>,

    ui_state: UiState,
    pub element_manager: ElementManager,
}

impl RenderManager {
    pub async fn new(window: &Window, element_manager: ElementManager) -> RenderManager {
        let mut render_cx = RenderContext::new().unwrap();

        let size = window.inner_size();
        let surface = render_cx
            .create_surface(&window, size.width, size.height)
            .await
            .unwrap();

        let device_handle = &render_cx.devices[surface.dev_id];

        let options = RendererOptions {
            surface_format: Some(
                surface
                    .surface
                    .get_current_texture()
                    .unwrap()
                    .texture
                    .format(),
            ),
            timestamp_period: 0.0,
        };
        let renderer = Renderer::new(&device_handle.device, &options).unwrap();
        let vello_surface =
            render_cx.devices[surface.dev_id]
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    label: None,
                    mip_level_count: 1,
                    sample_count: 1,
                    size: wgpu::Extent3d {
                        width: surface.config.width - 500,
                        height: surface.config.height,
                        depth_or_array_layers: 1,
                    },
                    usage: wgpu::TextureUsages::STORAGE_BINDING
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &surface.config.view_formats,
                });
        let convert_surface =
            render_cx.devices[surface.dev_id]
                .device
                .create_texture(&wgpu::TextureDescriptor {
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    label: None,
                    mip_level_count: 1,
                    sample_count: 1,
                    size: wgpu::Extent3d {
                        width: surface.config.width - 500,
                        height: surface.config.height,
                        depth_or_array_layers: 1,
                    },
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &surface.config.view_formats,
                });

        let converter =
            TextureConverter::new(&device_handle.device, &vello_surface, surface.format);

        let mut gui_renderer = egui_wgpu::Renderer::new(
            &render_cx.devices[surface.dev_id].device,
            surface.format,
            None,
            1,
        );

        let texture = gui_renderer.register_native_texture(
            &render_cx.devices[surface.dev_id].device,
            &convert_surface.create_view(&wgpu::TextureViewDescriptor::default()),
            wgpu::FilterMode::Linear,
        );

        RenderManager {
            ctx: render_cx,
            renderer,

            main_surface: surface,
            vello_surface,
            convert_surface,
            converter,
            gui_renderer,
            gui_output: Default::default(),

            ui_state: UiState::new(texture),

            element_manager,
        }
    }

    fn gui(&mut self, ctx: &egui::Context) {
        egui::SidePanel::right("egui_demo_panel")
            // .resizable(false)
            .default_width(150.0)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("✒ egui demos");
                });

                ui.separator();

                use egui::special_emojis::{GITHUB, TWITTER};
                ui.hyperlink_to(
                    format!("{GITHUB} egui on GitHub"),
                    "https://github.com/emilk/egui",
                );
                ui.hyperlink_to(
                    format!("{TWITTER} @ernerfeldt"),
                    "https://twitter.com/ernerfeldt",
                );

                ui.separator();

                // self.demo_list_ui(ui);
            });

        egui::CentralPanel::default()
            .frame(Frame::none())
            .show(ctx, |ui| {
                // ui.painter().rect_filled(
                //                     ui.available_rect_before_wrap(),
                //                     0.0,
                //                     Color32::from_rgb(0, 128, 0),
                //                 )

                let size = ui.available_size();
                self.resize_viewport(size.x as u32, size.y as u32);
                self.draw();
                egui::Image::new(SizedTexture::new(self.ui_state.viewport_tex, size))
                    .paint_at(ui, ui.available_rect_before_wrap());
            });
        // ui.text
    }

    pub fn update_gui(
        &mut self,
        platform: &mut egui_winit::State,
        context: &egui::Context,
        window: &Window,
    ) {
        let input = platform.take_egui_input(window);

        let output = context.run(input, |ctx| self.gui(ctx));

        for delta in &output.textures_delta.set {
            let device = &self.ctx.devices[self.main_surface.dev_id];

            self.gui_renderer
                .update_texture(&device.device, &device.queue, delta.0, &delta.1);
        }

        self.gui_output = context.tessellate(output.shapes);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let device_handle = &self.ctx.devices[self.main_surface.dev_id];
        let mut config = self.main_surface.config.clone();
        config.width = width;
        config.height = height;

        self.main_surface
            .surface
            .configure(&device_handle.device, &config);

        self.main_surface.config = config;
        self.resize_viewport(width, height);
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let device_handle = &self.ctx.devices[self.main_surface.dev_id];
        self.vello_surface = device_handle
            .device
            .create_texture(&wgpu::TextureDescriptor {
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                label: None,
                mip_level_count: 1,
                sample_count: 1,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &self.main_surface.config.view_formats,
            });
        self.convert_surface = device_handle
            .device
            .create_texture(&wgpu::TextureDescriptor {
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8Unorm,
                label: None,
                mip_level_count: 1,
                sample_count: 1,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &self.main_surface.config.view_formats,
            });

        self.gui_renderer.free_texture(&self.ui_state.viewport_tex);
        self.ui_state.viewport_tex = self.gui_renderer.register_native_texture(
            &device_handle.device,
            &self
                .convert_surface
                .create_view(&wgpu::TextureViewDescriptor::default()),
            wgpu::FilterMode::Linear,
        );

        self.converter
            .recreate(&device_handle.device, &self.vello_surface);
    }

    pub fn draw(&mut self) {
        let mut scene = Scene::new();

        let width = self.vello_surface.width();
        let height = self.vello_surface.height();
        let device_handle = &self.ctx.devices[self.main_surface.dev_id];
        let mut builder = SceneBuilder::for_scene(&mut scene);

        let bounds = Rect::from_origin_size((0.0, 0.0), (width as f64, height as f64));

        builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(Color::WHITE_SMOKE),
            None,
            &bounds,
        );

        self.element_manager.draw(&mut builder, &bounds, 0 as i8);

        let surface_texture = self
            .vello_surface
            .create_view(&wgpu::TextureViewDescriptor::default());

        let params = RenderParams {
            width,
            height,
            base_color: Color::BLACK,
        };

        block_on_wgpu(
            &device_handle.device,
            self.renderer.render_to_texture_async(
                &device_handle.device,
                &device_handle.queue,
                &scene,
                &surface_texture,
                &params,
            ),
        )
        .expect("failed to render to surface");

        self.converter.convert(
            &device_handle.device,
            &device_handle.queue,
            &self.convert_surface,
        );

        device_handle.device.poll(wgpu::Maintain::Poll);
    }

    pub fn present(&mut self) {
        render_gui_to_surface(
            &self.ctx.devices[self.main_surface.dev_id].device,
            &self.ctx.devices[self.main_surface.dev_id].queue,
            &self.main_surface,
            &mut self.gui_renderer,
            &self.gui_output,
        );
    }
}

pub fn render_gui_to_surface(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    surface: &RenderSurface,
    renderer: &mut egui_wgpu::Renderer,
    primitives: &[egui::ClippedPrimitive],
) {
    let output = surface.surface.get_current_texture().unwrap();
    let view = output
        .texture
        .create_view(&wgpu::TextureViewDescriptor::default());

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

    // Draw gui
    let screen = egui_wgpu::renderer::ScreenDescriptor {
        pixels_per_point: 1.0,
        size_in_pixels: [surface.config.width, surface.config.height],
    };

    renderer.update_buffers(&device, &queue, &mut encoder, &primitives, &screen);

    {
        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("compositor-gui-render-pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: true,
                },
            })],
            depth_stencil_attachment: None,
        });

        renderer.render(&mut render_pass, &primitives, &screen)
    }

    queue.submit(Some(encoder.finish()));
    output.present();
}

pub struct TextureConverter {
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    texture_bind_group: wgpu::BindGroup,
}

impl TextureConverter {
    pub fn new(
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
    ) -> TextureConverter {
        let shader_source = include_str!("shaders/texture_convert.wgsl");
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compositor-shader"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("textures-bind-group-layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        count: None,
                    },
                ],
            });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textures-bind-group"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
            layout: &texture_bind_group_layout,
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compositor-pipeline-layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("compositor-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                entry_point: "fs_main",
                module: &shader,
                targets: &[Some(wgpu::ColorTargetState {
                    blend: None,
                    format,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: None,
        });

        TextureConverter {
            pipeline,
            texture_layout: texture_bind_group_layout,
            texture_bind_group,
        }
    }

    pub fn recreate(&mut self, device: &wgpu::Device, texture: &wgpu::Texture) {
        let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        self.texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("textures-bind-group"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&wgpu::TextureViewDescriptor::default()),
                    ),
                },
            ],
            layout: &self.texture_layout,
        });
    }

    pub fn convert(&self, device: &wgpu::Device, queue: &wgpu::Queue, surface: &wgpu::Texture) {
        let view = surface.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("compositor-render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::RED),
                        store: true,
                    },
                })],
                depth_stencil_attachment: None,
            });

            render_pass.set_pipeline(&self.pipeline);
            render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
            render_pass.draw(0..4, 0..1);
        }

        queue.submit(Some(encoder.finish()));
    }
}
