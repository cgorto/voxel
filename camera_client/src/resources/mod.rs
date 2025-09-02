use crate::prelude::*;
use bevy::render::extract_resource::ExtractResource;

#[derive(Resource, Clone, ExtractResource)]
pub struct CameraTextures {
    pub current: Handle<Image>,
    pub prev: Handle<Image>,
}
