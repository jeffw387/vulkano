use super::pool::standard::{StandardCommandPool, StandardCommandPoolBuilder};
use super::sys::{Flags, Kind, UnsafeCommandBuffer, UnsafeCommandBufferBuilder, UnsafeCommandBufferBuilderBufferImageCopy, UnsafeCommandBufferBuilderImageAspect};
use crate::device::Queue;
use crate::{
    buffer::{BufferUsage, CpuAccessibleBuffer, BufferAccess},
    device::Device,
    format::FormatDesc,
    image::{immutable2::ImmutableImage, Dimensions, MipmapsCount, ImageUsage, ImageLayout, ImageAccess},
    memory::DeviceMemoryAllocError,
    sync::FenceFuture,
    format::{PossibleDepthFormatDesc, PossibleStencilFormatDesc},
    OomError,
};
use std::fmt::Display;
use std::sync::Arc;

enum Error {
    TextureUpload(String),
    DeviceMemoryAllocError(DeviceMemoryAllocError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TextureUpload(s) => write!(f, "Error uploading texture to GPU: {}", s),
            Error::DeviceMemoryAllocError(e) => write!(f, "Device memory allocation error: {}", e),
        }
    }
}

impl From<DeviceMemoryAllocError> for Error {
    fn from(e: DeviceMemoryAllocError) -> Error {
        Error::DeviceMemoryAllocError(e)
    }
}

pub trait CommandBuffer<F>
where
    F: std::future::Future,
{
    fn enqueue(queue: Arc<Queue>) -> F;

    fn submit(queue: Arc<Queue>) -> F;
}

pub struct TextureUpload {
    inner: UnsafeCommandBuffer<StandardCommandPool>,
}

impl<F> CommandBuffer<FenceFuture<ImmutableImage<F>>> for TextureUpload
where
    F: FormatDesc,
{
    fn enqueue(queue: Arc<Queue>) -> FenceFuture<ImmutableImage<F>> {
        unimplemented!()
    }

    fn submit(queue: Arc<Queue>) -> FenceFuture<ImmutableImage<F>> {
        unimplemented!()
    }
}




#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn make_cb() {
        let (device, queue) = gfx_dev_and_queue!();
        unsafe {
            let cb = UnsafeCommandBufferBuilder::new(
                &Device::standard_command_pool(&device, queue.family()),
                Kind::primary(),
                Flags::OneTimeSubmit,
            )
            .unwrap()
            .build();
        }
    }
}
