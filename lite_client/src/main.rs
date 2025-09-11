use nokhwa::{
    pixel_format::RgbAFormat,
    utils::{CameraIndex, RequestedFormat},
    *,
};
use wgpu::{Device, Queue};

fn main() {
    println!("Hello, world!");
}

fn init_camera() {
    let index = CameraIndex::Index(0);
    let requested =
        RequestedFormat::new::<RgbAFormat>(utils::RequestedFormatType::AbsoluteHighestFrameRate);
    let camera = Camera::new(index, requested).unwrap();
    let fps = camera.frame_rate() as f64;
}

fn init_textures(cam: &mut Camera, device: &Device, queue: &Queue) {
    let _ = cam.open_stream();
    let mut base_frame: wgpu::Texture = cam
        .frame_texture::<RgbAFormat>(device, queue, Some("base_frame"))
        .unwrap();
    let size = base_frame.size();
    let previous_frame = Texture;
    //uhhh clone it a couple times?
}
