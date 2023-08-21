use vello::{
    block_on_wgpu,
    kurbo::{Affine, Rect},
    peniko::{Brush, Color, Fill},
    util::{RenderContext, RenderSurface},
    RenderParams, Renderer, RendererOptions, Scene, SceneBuilder,
};
use winit::window::Window;

use crate::element::ElementManager;

pub struct RenderManager {
    ctx: RenderContext,
    surface: RenderSurface,
    renderer: Renderer,

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

        RenderManager {
            ctx: render_cx,
            renderer,
            surface,

            element_manager,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let device_handle = &self.ctx.devices[self.surface.dev_id];
        let mut config = self.surface.config.clone();
        config.width = width;
        config.height = height;

        self.surface
            .surface
            .configure(&device_handle.device, &config);

        self.surface.config = config;
    }

    pub fn draw(&mut self) {
        let mut scene = Scene::new();

        let width = self.surface.config.width;
        let height = self.surface.config.height;
        let device_handle = &self.ctx.devices[self.surface.dev_id];
        let mut builder = SceneBuilder::for_scene(&mut scene);

        let bounds = Rect::from_origin_size((0.0, 0.0), (width as f64, height as f64));

        builder.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &Brush::Solid(Color::WHITE_SMOKE),
            None,
            &bounds
        );

        self.element_manager.draw(&mut builder, &bounds);

        let surface_texture = self
            .surface
            .surface
            .get_current_texture()
            .expect("failed to get surface texture");

        let params = RenderParams {
            width,
            height,
            base_color: Color::BLACK,
        };

        block_on_wgpu(
            &device_handle.device,
            self.renderer.render_to_surface_async(
                &device_handle.device,
                &device_handle.queue,
                &scene,
                &surface_texture,
                &params,
            ),
        )
        .expect("failed to render to surface");

        surface_texture.present();
        device_handle.device.poll(wgpu::Maintain::Poll);
    }
}
