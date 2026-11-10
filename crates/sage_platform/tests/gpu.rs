//! The GPU renderer against the CPU reference rasterizer.
//!
//! Needs a Vulkan device. Without one the tests print SKIP and pass, unless
//! `SAGE_REQUIRE_GPU=1` is set (use that where a GPU is expected).

use sage_assets::{BitmapFont, Image};
use sage_core::StableId;
use sage_core::host::{DrawKind, DrawRect, Frame};
use sage_platform::gpu::GpuRenderer;
use sage_platform::sprite::{self, ScaleMode, Skin, TextureId};

/// SDL can be initialized once at a time, so the tests take turns.
static SDL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn renderer() -> Option<(std::sync::MutexGuard<'static, ()>, GpuRenderer)> {
    let guard = SDL.lock().unwrap_or_else(|e| e.into_inner());
    match GpuRenderer::offscreen() {
        Ok(r) => Some((guard, r)),
        Err(e) => {
            assert!(
                std::env::var("SAGE_REQUIRE_GPU").is_err(),
                "SAGE_REQUIRE_GPU is set but there is no GPU: {e}"
            );
            eprintln!("SKIP: no GPU device ({e})");
            None
        }
    }
}

fn frame() -> Frame {
    let rect = |id, kind, center: [f32; 2], half: [f32; 2]| DrawRect {
        id: StableId(id),
        kind,
        center,
        half,
    };
    Frame {
        tick: 0,
        world_size: [160.0, 90.0],
        rects: vec![
            rect(1, DrawKind::Court, [80.0, 45.0], [80.0, 45.0]),
            rect(10, DrawKind::Wall, [80.0, 88.0], [80.0, 2.0]),
            rect(30, DrawKind::Paddle, [5.0, 40.0], [1.5, 10.0]),
            rect(40, DrawKind::Ball, [80.0, 45.0], [3.0, 3.0]),
        ],
        hud: vec!["12  :  7".into(), "Playing".into()],
    }
}

/// A 3x3 sprite: opaque yellow with a transparent corner and a half-alpha pixel.
/// The ball is 12 px wide in these tests, so a texel is 4 px: no texel edge falls
/// on a pixel center, where the CPU and GPU would break the tie differently.
fn ball_sprite() -> Image {
    let mut img = Image::solid(3, 3, [255, 200, 60, 255]);
    img.set_pixel(0, 0, [255, 200, 60, 0]);
    img.set_pixel(2, 2, [255, 200, 60, 128]);
    img
}

fn compare(gpu: &Image, cpu: &[[u8; 4]]) -> (usize, u8) {
    let mut bad = 0;
    let mut worst = 0u8;
    for (g, c) in gpu.rgba.as_chunks::<4>().0.iter().zip(cpu) {
        let d = (0..3).map(|i| g[i].abs_diff(c[i])).max().unwrap();
        worst = worst.max(d);
        if d > 2 {
            if bad < 12 {
                eprintln!("pixel {}: gpu {g:?} cpu {c:?}", bad);
            }
            bad += 1;
        }
    }
    (bad, worst)
}

#[test]
fn gpu_frame_matches_the_cpu_reference() {
    let Some((_guard, mut gpu)) = renderer() else {
        return;
    };
    let size = [480u32, 300];
    let font = BitmapFont::builtin();
    let batch = sprite::compose(
        &frame(),
        [size[0] as f32, size[1] as f32],
        ScaleMode::PixelPerfect,
        &font,
        &Skin::default(),
    );
    let image = gpu.render_to_image(&batch, size).unwrap();
    let textures = [Image::solid(1, 1, [255; 4]), font.atlas.clone()];
    let cpu = sprite::rasterize(&batch, &textures, [0, 0, 0, 255], size);
    let (bad, worst) = compare(&image, &cpu);
    assert_eq!(bad, 0, "{bad} pixels differ by more than 2 (worst {worst})");
    // And it drew something: the paddle's pixels are in the readback.
    let counts = sage_platform::count_kind_pixels(&image);
    for (kind, n) in counts {
        if kind != DrawKind::Goal {
            assert!(n > 0, "no {kind:?} pixels");
        }
    }
}

#[test]
fn gpu_textured_sprites_and_alpha_match_the_cpu_reference() {
    let Some((_guard, mut gpu)) = renderer() else {
        return;
    };
    let size = [480u32, 300];
    let font = BitmapFont::builtin();
    let sprite_tex = gpu.add_texture(&ball_sprite()).unwrap();
    assert_eq!(sprite_tex, TextureId(2));
    let skin = Skin {
        sprites: vec![(DrawKind::Ball, sprite_tex)],
    };
    let batch = sprite::compose(
        &frame(),
        [size[0] as f32, size[1] as f32],
        ScaleMode::PixelPerfect,
        &font,
        &skin,
    );
    let image = gpu.render_to_image(&batch, size).unwrap();
    let textures = [
        Image::solid(1, 1, [255; 4]),
        font.atlas.clone(),
        ball_sprite(),
    ];
    let cpu = sprite::rasterize(&batch, &textures, [0, 0, 0, 255], size);
    let (bad, worst) = compare(&image, &cpu);
    assert_eq!(bad, 0, "{bad} pixels differ by more than 2 (worst {worst})");
    // The transparent corner shows the court behind it; the sprite body is exact.
    let ball_px = sage_platform::count_kind_pixels(&image)
        .into_iter()
        .find(|(k, _)| *k == DrawKind::Ball)
        .unwrap()
        .1;
    assert!(ball_px > 0);
}

#[test]
fn the_same_batch_renders_identically_twice_and_after_growth() {
    let Some((_guard, mut gpu)) = renderer() else {
        return;
    };
    let size = [320u32, 200];
    let font = BitmapFont::builtin();
    let small = sprite::compose(
        &frame(),
        [320.0, 200.0],
        ScaleMode::PixelPerfect,
        &font,
        &Skin::default(),
    );
    let a = gpu.render_to_image(&small, size).unwrap();
    // A much larger batch forces the vertex and index buffers to grow.
    let mut big = small.clone();
    for i in 0..2000 {
        big.solid(
            [(i % 300) as f32, (i % 190) as f32],
            [2.0, 2.0],
            [0.2, 0.4, 0.6, 1.0],
        );
    }
    let b = gpu.render_to_image(&big, size).unwrap();
    assert_ne!(a, b);
    let again = gpu.render_to_image(&small, size).unwrap();
    assert_eq!(a, again);
}
