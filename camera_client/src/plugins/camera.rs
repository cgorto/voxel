use crate::prelude::*;
use bevy::{
    asset::RenderAssetUsages,
    render::{
        extract_resource::ExtractResource,
        render_resource::{
            Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
        },
        renderer::{RenderDevice, RenderQueue},
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
        app.insert_non_send_resource(camera)
            .add_systems(Update, camera_to_texture)
            .add_systems(Startup, setup);
    }
}

pub fn setup(
    mut commands: Commands,
    mut cam: NonSendMut<nokhwa::Camera>,
    mut images: ResMut<Assets<Image>>,
) {
    commands.spawn(Camera2d);
    cam.open_stream();
    let base_frame = cam.frame().unwrap().decode_image::<RgbAFormat>().unwrap();
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
    image.texture_descriptor.usage =
        TextureUsages::COPY_DST | TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT;
    let prev = images.add(image.clone());
    let current = images.add(image);
    commands.insert_resource(CameraTextures {
        current: current.clone(),
        prev: prev.clone(),
    });

    commands.spawn(Sprite {
        image: current.clone(),
        custom_size: Some(vec2(texture_size.width as f32, texture_size.height as f32)),
        ..default()
    });
}

pub fn camera_to_texture(
    mut cam: NonSendMut<nokhwa::Camera>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
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
    }
}
