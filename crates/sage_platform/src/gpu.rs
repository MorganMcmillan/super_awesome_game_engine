//! The GPU renderer: SDL's GPU API with the sprite shaders from
//! `tools/shaderc`. It draws the `SpriteBatch` that `sprite::compose` builds.
//!
//! Only SPIR-V is offered, so SDL uses its Vulkan backend. A machine
//! without Vulkan fails `GpuRenderer::new` with an error, and the caller
//! falls back to the simple Render API (`SdlRenderer`).

use sage_assets::{BitmapFont, Image};
use sage_core::host::{Frame, UiFrame};
use sdl3::gpu::{
    BlendFactor, BlendOp, BufferBinding, BufferRegion, BufferUsageFlags, ColorTargetBlendState,
    ColorTargetDescription, ColorTargetInfo, CullMode, Device, FillMode, Filter, GraphicsPipeline,
    GraphicsPipelineTargetInfo, IndexElementSize, LoadOp, PrimitiveType, RasterizerState,
    SampleCount, Sampler, SamplerAddressMode, SamplerCreateInfo, SamplerMipmapMode, ShaderFormat,
    ShaderStage, StoreOp, Texture, TextureCreateInfo, TextureFormat, TextureRegion,
    TextureSamplerBinding, TextureTransferInfo, TextureType, TextureUsage, TransferBufferLocation,
    TransferBufferUsage, VertexAttribute, VertexBufferDescription, VertexElementFormat,
    VertexInputRate, VertexInputState,
};
use sdl3::pixels::Color;
use sdl3::video::Window;

use crate::sprite::{self, FONT, ScaleMode, Skin, SpriteBatch, TextureId, Vertex, WHITE};
use crate::{BACKGROUND, Renderer};

const VERT_SPV: &[u8] = include_bytes!("../shaders/sprite.vert.spv");
const FRAG_SPV: &[u8] = include_bytes!("../shaders/sprite.frag.spv");

fn err(e: impl ToString) -> String {
    e.to_string()
}

fn bytes_of<T>(slice: &[T]) -> &[u8] {
    // Only used for `Vertex` (repr(C), all f32, no padding) and `u32`.
    unsafe { std::slice::from_raw_parts(slice.as_ptr().cast(), std::mem::size_of_val(slice)) }
}

pub struct GpuRenderer {
    device: Device,
    window: Option<Window>,
    /// Keeps SDL's video subsystem (and so the Vulkan loader) alive for a
    /// renderer made by `offscreen`.
    _video: Option<(sdl3::Sdl, sdl3::VideoSubsystem)>,
    pipelines: Vec<(TextureFormat, GraphicsPipeline)>,
    sampler: Sampler,
    textures: Vec<Texture<'static>>,
    font: BitmapFont,
    pub skin: Skin,
    pub mode: ScaleMode,
    vertex_buffer: Option<(sdl3::gpu::Buffer, u32)>,
    index_buffer: Option<(sdl3::gpu::Buffer, u32)>,
}

impl GpuRenderer {
    /// A renderer that draws into `window`'s swapchain.
    pub fn new(window: Window) -> Result<GpuRenderer, String> {
        let device = Device::new(ShaderFormat::SPIRV, cfg!(debug_assertions))
            .map_err(err)?
            .with_window(&window)
            .map_err(err)?;
        let mut r = Self::with_device(device)?;
        r.window = Some(window);
        Ok(r)
    }

    /// A renderer with no window, for `render_to_image`.
    pub fn offscreen() -> Result<GpuRenderer, String> {
        let sdl = sdl3::init().map_err(err)?;
        let video = sdl.video().map_err(err)?;
        let device = Device::new(ShaderFormat::SPIRV, cfg!(debug_assertions)).map_err(err)?;
        let mut r = Self::with_device(device)?;
        r._video = Some((sdl, video));
        Ok(r)
    }

    fn with_device(device: Device) -> Result<GpuRenderer, String> {
        let sampler = device
            .create_sampler(
                SamplerCreateInfo::new()
                    .with_min_filter(Filter::Nearest)
                    .with_mag_filter(Filter::Nearest)
                    .with_mipmap_mode(SamplerMipmapMode::Nearest)
                    .with_address_mode_u(SamplerAddressMode::ClampToEdge)
                    .with_address_mode_v(SamplerAddressMode::ClampToEdge)
                    .with_address_mode_w(SamplerAddressMode::ClampToEdge),
            )
            .map_err(err)?;
        let font = BitmapFont::builtin();
        let mut r = GpuRenderer {
            device,
            window: None,
            _video: None,
            pipelines: vec![],
            sampler,
            textures: vec![],
            font: font.clone(),
            skin: Skin::default(),
            mode: ScaleMode::PixelPerfect,
            vertex_buffer: None,
            index_buffer: None,
        };
        // Slots 0 and 1 are fixed: see `sprite::WHITE` and `sprite::FONT`.
        assert_eq!(r.add_texture(&Image::solid(1, 1, [255; 4]))?, WHITE);
        assert_eq!(r.add_texture(&font.atlas)?, FONT);
        Ok(r)
    }

    pub fn backend_name(&self) -> String {
        // SDL reports the driver per device through a property; the format set
        // tells us enough for logs.
        format!(
            "sdl-gpu (SPIR-V, formats {:?})",
            self.device.get_shader_formats()
        )
    }

    /// Uploads an image as a texture and returns its slot.
    pub fn add_texture(&mut self, image: &Image) -> Result<TextureId, String> {
        let texture = self
            .device
            .create_texture(
                TextureCreateInfo::new()
                    .with_format(TextureFormat::R8g8b8a8Unorm)
                    .with_type(TextureType::_2D)
                    .with_width(image.width)
                    .with_height(image.height)
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_usage(TextureUsage::SAMPLER),
            )
            .map_err(err)?;
        let transfer = self
            .device
            .create_transfer_buffer()
            .with_size(image.rgba.len() as u32)
            .with_usage(TransferBufferUsage::UPLOAD)
            .build()
            .map_err(err)?;
        let mut map = transfer.map::<u8>(&self.device, false);
        map.mem_mut().copy_from_slice(&image.rgba);
        map.unmap();
        let cmd = self.device.acquire_command_buffer().map_err(err)?;
        let pass = self.device.begin_copy_pass(&cmd).map_err(err)?;
        pass.upload_to_gpu_texture(
            TextureTransferInfo::new()
                .with_transfer_buffer(&transfer)
                .with_offset(0),
            TextureRegion::new()
                .with_texture(&texture)
                .with_layer(0)
                .with_width(image.width)
                .with_height(image.height)
                .with_depth(1),
            false,
        );
        self.device.end_copy_pass(pass);
        cmd.submit().map_err(err)?;
        self.textures.push(texture);
        Ok(TextureId(self.textures.len() as u32 - 1))
    }

    fn pipeline_index(&mut self, format: TextureFormat) -> Result<usize, String> {
        if let Some(i) = self.pipelines.iter().position(|p| p.0 == format) {
            return Ok(i);
        }
        let vert = self
            .device
            .create_shader()
            .with_code(ShaderFormat::SPIRV, VERT_SPV, ShaderStage::Vertex)
            .with_entrypoint(c"main")
            .build()
            .map_err(err)?;
        let frag = self
            .device
            .create_shader()
            .with_code(ShaderFormat::SPIRV, FRAG_SPV, ShaderStage::Fragment)
            .with_samplers(1)
            .with_entrypoint(c"main")
            .build()
            .map_err(err)?;
        let f32s = |n: u32| n * std::mem::size_of::<f32>() as u32;
        let pipeline = self
            .device
            .create_graphics_pipeline()
            .with_primitive_type(PrimitiveType::TriangleList)
            .with_vertex_shader(&vert)
            .with_fragment_shader(&frag)
            .with_vertex_input_state(
                VertexInputState::new()
                    .with_vertex_buffer_descriptions(&[VertexBufferDescription::new()
                        .with_slot(0)
                        .with_pitch(std::mem::size_of::<Vertex>() as u32)
                        .with_input_rate(VertexInputRate::Vertex)
                        .with_instance_step_rate(0)])
                    .with_vertex_attributes(&[
                        VertexAttribute::new()
                            .with_location(0)
                            .with_buffer_slot(0)
                            .with_format(VertexElementFormat::Float2)
                            .with_offset(0),
                        VertexAttribute::new()
                            .with_location(1)
                            .with_buffer_slot(0)
                            .with_format(VertexElementFormat::Float2)
                            .with_offset(f32s(2)),
                        VertexAttribute::new()
                            .with_location(2)
                            .with_buffer_slot(0)
                            .with_format(VertexElementFormat::Float4)
                            .with_offset(f32s(4)),
                    ]),
            )
            .with_rasterizer_state(
                RasterizerState::new()
                    .with_fill_mode(FillMode::Fill)
                    .with_cull_mode(CullMode::None),
            )
            .with_target_info(
                GraphicsPipelineTargetInfo::new().with_color_target_descriptions(&[
                    ColorTargetDescription::new()
                        .with_format(format)
                        .with_blend_state(
                            ColorTargetBlendState::new()
                                .with_enable_blend(true)
                                .with_src_color_blendfactor(BlendFactor::SrcAlpha)
                                .with_dst_color_blendfactor(BlendFactor::OneMinusSrcAlpha)
                                .with_color_blend_op(BlendOp::Add)
                                .with_src_alpha_blendfactor(BlendFactor::One)
                                .with_dst_alpha_blendfactor(BlendFactor::OneMinusSrcAlpha)
                                .with_alpha_blend_op(BlendOp::Add),
                        ),
                ]),
            )
            .build()
            .map_err(err)?;
        self.pipelines.push((format, pipeline));
        Ok(self.pipelines.len() - 1)
    }

    /// Grows (never shrinks) the vertex and index buffers to hold `batch`.
    fn ensure_buffers(&mut self, vbytes: u32, ibytes: u32) -> Result<(), String> {
        let grow = |cur: &Option<(sdl3::gpu::Buffer, u32)>, need: u32| {
            cur.as_ref().is_none_or(|c| c.1 < need)
        };
        if grow(&self.vertex_buffer, vbytes) {
            let cap = vbytes.next_power_of_two().max(4096);
            let b = self
                .device
                .create_buffer()
                .with_usage(BufferUsageFlags::VERTEX)
                .with_size(cap)
                .build()
                .map_err(err)?;
            self.vertex_buffer = Some((b, cap));
        }
        if grow(&self.index_buffer, ibytes) {
            let cap = ibytes.next_power_of_two().max(4096);
            let b = self
                .device
                .create_buffer()
                .with_usage(BufferUsageFlags::INDEX)
                .with_size(cap)
                .build()
                .map_err(err)?;
            self.index_buffer = Some((b, cap));
        }
        Ok(())
    }

    /// Uploads `batch` and draws it into `target` on `cmd`.
    fn record(
        &mut self,
        cmd: &sdl3::gpu::CommandBuffer,
        target: &Texture<'_>,
        format: TextureFormat,
        batch: &SpriteBatch,
    ) -> Result<(), String> {
        let (vb, ib) = (bytes_of(&batch.vertices), bytes_of(&batch.indices));
        let pipeline = self.pipeline_index(format)?;
        if !vb.is_empty() {
            self.ensure_buffers(vb.len() as u32, ib.len() as u32)?;
            let transfer = self
                .device
                .create_transfer_buffer()
                .with_size((vb.len() + ib.len()) as u32)
                .with_usage(TransferBufferUsage::UPLOAD)
                .build()
                .map_err(err)?;
            let mut map = transfer.map::<u8>(&self.device, false);
            map.mem_mut()[..vb.len()].copy_from_slice(vb);
            map.mem_mut()[vb.len()..].copy_from_slice(ib);
            map.unmap();
            let copy = self.device.begin_copy_pass(cmd).map_err(err)?;
            let (vbuf, ibuf) = (
                &self.vertex_buffer.as_ref().expect("ensured").0,
                &self.index_buffer.as_ref().expect("ensured").0,
            );
            copy.upload_to_gpu_buffer(
                TransferBufferLocation::new()
                    .with_transfer_buffer(&transfer)
                    .with_offset(0),
                BufferRegion::new()
                    .with_buffer(vbuf)
                    .with_offset(0)
                    .with_size(vb.len() as u32),
                true,
            );
            copy.upload_to_gpu_buffer(
                TransferBufferLocation::new()
                    .with_transfer_buffer(&transfer)
                    .with_offset(vb.len() as u32),
                BufferRegion::new()
                    .with_buffer(ibuf)
                    .with_offset(0)
                    .with_size(ib.len() as u32),
                true,
            );
            self.device.end_copy_pass(copy);
        }
        let targets = [ColorTargetInfo::default()
            .with_texture(target)
            .with_load_op(LoadOp::CLEAR)
            .with_store_op(StoreOp::STORE)
            .with_clear_color(Color::RGBA(BACKGROUND.r, BACKGROUND.g, BACKGROUND.b, 255))];
        let pass = self
            .device
            .begin_render_pass(cmd, &targets, None)
            .map_err(err)?;
        if !vb.is_empty() {
            pass.bind_graphics_pipeline(&self.pipelines[pipeline].1);
            pass.bind_vertex_buffers(
                0,
                &[BufferBinding::new()
                    .with_buffer(&self.vertex_buffer.as_ref().expect("ensured").0)
                    .with_offset(0)],
            );
            pass.bind_index_buffer(
                &BufferBinding::new()
                    .with_buffer(&self.index_buffer.as_ref().expect("ensured").0)
                    .with_offset(0),
                IndexElementSize::_32BIT,
            );
            for run in &batch.runs {
                let texture = self
                    .textures
                    .get(run.texture.0 as usize)
                    .ok_or_else(|| format!("no texture {}", run.texture.0))?;
                pass.bind_fragment_samplers(
                    0,
                    &[TextureSamplerBinding::new()
                        .with_texture(texture)
                        .with_sampler(&self.sampler)],
                );
                pass.draw_indexed_primitives(run.index_count, 1, run.first_index, 0, 0);
            }
        }
        self.device.end_render_pass(pass);
        Ok(())
    }

    /// Renders `batch` into an RGBA texture of `size` and reads it back.
    /// Slow (waits for the GPU); meant for tests and screenshots.
    pub fn render_to_image(
        &mut self,
        batch: &SpriteBatch,
        size: [u32; 2],
    ) -> Result<Image, String> {
        use sdl3::sys::gpu as sys;
        let format = TextureFormat::R8g8b8a8Unorm;
        let target = self
            .device
            .create_texture(
                TextureCreateInfo::new()
                    .with_format(format)
                    .with_type(TextureType::_2D)
                    .with_width(size[0])
                    .with_height(size[1])
                    .with_layer_count_or_depth(1)
                    .with_num_levels(1)
                    .with_sample_count(SampleCount::NoMultiSampling)
                    .with_usage(TextureUsage::COLOR_TARGET),
            )
            .map_err(err)?;
        let bytes = size[0] * size[1] * 4;
        let download = self
            .device
            .create_transfer_buffer()
            .with_size(bytes)
            .with_usage(TransferBufferUsage::DOWNLOAD)
            .build()
            .map_err(err)?;
        let cmd = self.device.acquire_command_buffer().map_err(err)?;
        self.record(&cmd, &target, format, batch)?;
        let copy = self.device.begin_copy_pass(&cmd).map_err(err)?;
        // The wrapper has no download call; use the raw one.
        unsafe {
            let region = sys::SDL_GPUTextureRegion {
                texture: target.raw(),
                mip_level: 0,
                layer: 0,
                x: 0,
                y: 0,
                z: 0,
                w: size[0],
                h: size[1],
                d: 1,
            };
            let info = sys::SDL_GPUTextureTransferInfo {
                transfer_buffer: download.raw(),
                offset: 0,
                pixels_per_row: size[0],
                rows_per_layer: size[1],
            };
            sys::SDL_DownloadFromGPUTexture(copy.raw(), &region, &info);
        }
        self.device.end_copy_pass(copy);
        let fence = cmd.submit_and_acquire_fence(&self.device).map_err(err)?;
        self.device.wait_fences(true, &[fence]).map_err(err)?;
        let map = download.map::<u8>(&self.device, false);
        let rgba = map.mem().to_vec();
        map.unmap();
        Image::new(size[0], size[1], rgba)
    }
}

impl GpuRenderer {
    /// Draws one frame into the window's swapchain; `make` builds the batch
    /// for the viewport (the window's size in pixels).
    fn present(
        &mut self,
        make: impl FnOnce(&GpuRenderer, [f32; 2]) -> SpriteBatch,
    ) -> Result<(), String> {
        let window = self.window.take().ok_or("this renderer has no window")?;
        let result = (|| {
            let mut cmd = self.device.acquire_command_buffer().map_err(err)?;
            let swapchain = cmd.wait_and_acquire_swapchain_texture(&window);
            let Ok(Some(swapchain)) = swapchain else {
                // Minimized or not ready: nothing to draw this frame.
                cmd.cancel();
                return Ok(());
            };
            let format = self.device.get_swapchain_texture_format(&window);
            let viewport = [swapchain.width() as f32, swapchain.height() as f32];
            let batch = make(self, viewport);
            self.record(&cmd, &swapchain, format, &batch)?;
            cmd.submit().map_err(err)
        })();
        self.window = Some(window);
        result
    }

    /// Draws a UI frame (`sprite::compose_ui`) into the window.
    pub fn draw_ui(&mut self, frame: &UiFrame) -> Result<(), String> {
        self.present(|r, viewport| sprite::compose_ui(frame, viewport, &r.font))
    }
}

impl Renderer for GpuRenderer {
    fn draw(&mut self, frame: &Frame) -> Result<(), String> {
        self.present(|r, viewport| sprite::compose(frame, viewport, r.mode, &r.font, &r.skin))
    }
}
