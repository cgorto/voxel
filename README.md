## Voxel Based Object Triangulation

This is a prototype implementation of a method to locate the position of a moving object in 3-dimensional space.

### Overview:

- Every pixel of every frame of a camera feed is compared to the pixel from the previous frame. If the grayscale difference between them is above a set threshold, then the difference is written to a new texture (diff function in client/assets/shaders/processing.wgsl).
- Then, for every pixel of this new texture that is above the threshold we raymarch into the voxel grid, marking every voxel hit. The method of "marking" can either be done using a 3D texture, or using a dynamic storage buffer with an atomic counter. The former is more ergonomic, but has explosive memory growth.
- Once the raymarching pass has finished, we readback the voxels that have been hit and send them to the central server for aggregation.
- Whenever the server receives data from a camera client, it adds the difference value from a marked voxel to the corresponding voxel in the world, along with a timestamp. If that voxel had a previous value, then it will apply an exponential decay according to when that voxel was last hit.
- The voxels with a value above a given threshold (say the top 1%) are considered to be the ones that are depicting a moving object.
