use crate::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    render::render_resource::{
        Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    },
};
use nokhwa::{pixel_format::RgbAFormat, utils::RequestedFormat, *};
pub struct VoxelCameraPlugin;

impl Plugin for VoxelCameraPlugin {
    fn build(&self, app: &mut App) {
        let index = utils::CameraIndex::Index(0);
        let requested = RequestedFormat::new::<RgbAFormat>(
            utils::RequestedFormatType::AbsoluteHighestFrameRate,
        );
        let camera = nokhwa::Camera::new(index, requested).unwrap();
        let fps = camera.frame_rate() as f64;
        info!("Setting FixedUpdate to {} hz", fps);
        app.insert_non_send_resource(camera)
            .add_systems(PreUpdate, new_frame_reset)
            .add_systems(FixedUpdate, camera_to_texture)
            .add_systems(Startup, setup)
            .insert_resource(FrameInfo::default())
            .insert_resource(Time::<Fixed>::from_hz(fps));
    }
}

pub fn setup(
    mut commands: Commands,
    mut cam: NonSendMut<nokhwa::Camera>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn(Camera2d);
    let _ = cam.open_stream();
    let base_frame = cam.frame().unwrap().decode_image::<RgbAFormat>().unwrap();
    let image_size = ivec2(base_frame.width() as i32, base_frame.height() as i32);
    let texture_size = Extent3d {
        width: base_frame.width(),
        height: base_frame.height(),
        depth_or_array_layers: 1,
    };
    let mut image = Image::new(
        texture_size,
        TextureDimension::D2,
        base_frame.to_vec(),
        TextureFormat::Rgba8Unorm,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage = TextureUsages::COPY_DST
        | TextureUsages::TEXTURE_BINDING
        | TextureUsages::RENDER_ATTACHMENT
        | TextureUsages::STORAGE_BINDING;
    let prev = images.add(image.clone());
    let current = images.add(image.clone());
    let display = images.add(image);
    commands.insert_resource(CameraTextures {
        current: current.clone(),
        prev: prev.clone(),
        size: image_size,
        new_frame: true,
    });
    commands.insert_resource(DisplayTexture {
        handle: display.clone(),
    });

    commands.spawn(Sprite {
        image: display.clone(),
        custom_size: Some(vec2(texture_size.width as f32, texture_size.height as f32)),
        ..default()
    });
}

pub fn new_frame_reset(mut cam_text: ResMut<CameraTextures>) {
    cam_text.new_frame = false;
}

pub fn camera_to_texture(
    mut cam: NonSendMut<nokhwa::Camera>,
    mut images: ResMut<Assets<Image>>,
    mut cam_text: ResMut<CameraTextures>,
) {
    cam.open_stream();
    let frame = cam.frame().unwrap().decode_image::<RgbAFormat>().unwrap();
    let texture_size = Extent3d {
        width: frame.width(),
        height: frame.height(),
        depth_or_array_layers: 1,
    };
    let current_handle = cam_text.prev.clone();

    cam_text.prev = cam_text.current.clone();
    if let Some(image) = images.get_mut(&current_handle) {
        image.data = Some(frame.to_vec());
        cam_text.new_frame = true;
        cam_text.current = current_handle;
    }
}
