use super::pool::standard::{StandardCommandPool, StandardCommandPoolBuilder};
use super::sys::{Flags, Kind, UnsafeCommandBuffer, UnsafeCommandBufferBuilder};
use crate::device::Queue;
use crate::{
    buffer::{BufferUsage, CpuAccessibleBuffer},
    device::Device,
    format::FormatDesc,
    image::Dimensions,
    OomError,
};
use std::fmt::Display;
use std::sync::Arc;

enum Error {
    TextureUpload(String),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TextureUpload(s) => write!(f, "Error uploading texture to GPU: {}", s),
        }
    }
}

pub struct TextureUpload<F, I> {
    queue: Arc<Queue>,
    inner: UnsafeCommandBufferBuilder<StandardCommandPoolBuilder>,
    dimensions: Option<Dimensions>,
    format: Option<F>,
    iterator: I,
}

impl<F, I> TextureUpload<F, I> {
    pub fn new<P>(queue: Arc<Queue>, iter: I) -> Result<Self, OomError>
    where
        P: Send + Sync + Clone + 'static,
        I: ExactSizeIterator<Item = P>,
    {
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
                iterator: iter,
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

    pub fn build(&mut self) -> UnsafeCommandBuffer<StandardCommandPool> {
        // self.inner.
        unimplemented!()
    }

    fn stage(&mut self) -> Result<(), Error> {
        let source = CpuAccessibleBuffer::from_iter(
            self.queue.device().clone(),
            BufferUsage::transfer_source(),
            false,
            self.iterator,
            
        );
        Ok(())
    }
}

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
