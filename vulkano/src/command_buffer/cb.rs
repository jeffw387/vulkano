use super::pool::standard::{StandardCommandPool, StandardCommandPoolBuilder};
use super::sys::{Flags, Kind, UnsafeCommandBuffer, UnsafeCommandBufferBuilder};
use crate::device::Queue;
use crate::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    device::Device,
    format::FormatDesc,
    image::{immutable2::ImmutableImage, Dimensions},
    memory::DeviceMemoryAllocError,
    sync::FenceFuture,
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

pub struct TextureUploadBuilder<F> {
    queue: Arc<Queue>,
    inner: UnsafeCommandBufferBuilder<StandardCommandPoolBuilder>,
    dimensions: Option<Dimensions>,
    format: Option<F>,
}

impl<F> TextureUploadBuilder<F> {
    pub fn new<P>(queue: Arc<Queue>) -> Result<Self, OomError> {
        unsafe {
            Ok(Self {
                queue: queue.clone(),
                inner: UnsafeCommandBufferBuilder::new(
                    &Device::standard_command_pool(queue.device(), queue.family()),
                    Kind::primary(),
                    Flags::OneTimeSubmit,
                )?,
                dimensions: None,
                format: None,
            })
        }
    }

    pub fn dimensions(&mut self, dimensions: Dimensions) {
        self.dimensions = Some(dimensions);
    }

    pub fn format(&mut self, format: F)
    where
        F: FormatDesc,
    {
        self.format = Some(format);
    }

    pub fn build_from_iter<P, I>(&mut self, iter: I) -> TextureUpload
    where
        P: Send + Sync + Clone + 'static,
        I: ExactSizeIterator<Item = P>,
    {
        let staging = self.stage(iter);
        self.build_from_buffer();
        unimplemented!()
    }

    pub fn build_from_buffer<B>(&mut self, buffer: B) -> TextureUpload {
        let image = ImmutableImage::uninitialized(
            self.queue.device(),
            self.dimensions,
            self.format,
            MipmapsCount::One,
            ImageUsage {
                transfer_destination: true,
                sampled: true,
                ..ImageUsage::default()
            },
            ImageLayout::TransferDstOptimal,
            [queue.clone()],
        );
        self.inner.copy_buffer_to_image(
            staging.clone(),
            image.clone(),
            ImageLayout::TransferDstOptimal,
            &[region].iter(),
        );

        unimplemented!()
    }

    fn stage<P, I>(&mut self, iter: I) -> Result<Arc<CpuAccessibleBuffer<[P]>>, Error>
    where
        P: Send + Sync + Clone + 'static,
        I: ExactSizeIterator<Item = P>,
    {
        CpuAccessibleBuffer::from_iter(
            self.queue.device().clone(),
            BufferUsage::transfer_source(),
            false,
            iter,
        )
        .map_err(Error::from)
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
