// Copyright (c) 2016 The vulkano developers
// Licensed under the Apache License, Version 2.0
// <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT
// license <LICENSE-MIT or http://opensource.org/licenses/MIT>,
// at your option. All files in the project carrying such
// notice may not be copied, modified, or distributed except
// according to those terms.

use smallvec::SmallVec;
use std::hash::Hash;
use std::hash::Hasher;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crate::buffer::BufferAccess;
use crate::buffer::BufferUsage;
use crate::buffer::CpuAccessibleBuffer;
use crate::buffer::TypedBufferAccess;
use crate::command_buffer::AutoCommandBuffer;
use crate::command_buffer::AutoCommandBufferBuilder;
use crate::command_buffer::CommandBuffer;
use crate::command_buffer::CommandBufferExecFuture;
use crate::device::Device;
use crate::device::Queue;
use crate::format::AcceptsPixels;
use crate::format::Format;
use crate::format::FormatDesc;
use crate::image::sys::ImageCreationError;
use crate::image::sys::UnsafeImage;
use crate::image::sys::UnsafeImageView;
use crate::image::traits::ImageAccess;
use crate::image::traits::ImageContent;
use crate::image::traits::ImageViewAccess;
use crate::image::Dimensions;
use crate::image::ImageInner;
use crate::image::ImageLayout;
use crate::image::ImageUsage;
use crate::image::MipmapsCount;
use crate::instance::QueueFamily;
use crate::memory::pool::AllocFromRequirementsFilter;
use crate::memory::pool::AllocLayout;
use crate::memory::pool::MappingRequirement;
use crate::memory::pool::MemoryPool;
use crate::memory::pool::MemoryPoolAlloc;
use crate::memory::pool::PotentialDedicatedAllocation;
use crate::memory::pool::StdMemoryPoolAlloc;
use crate::memory::DedicatedAlloc;
use crate::sync::AccessError;
use crate::sync::NowFuture;
use crate::sync::Sharing;

/// Image whose purpose is to be used for read-only purposes. You can write to the image once,
/// but then you must only ever read from it.
// TODO: type (2D, 3D, array, etc.) as template parameter
#[derive(Debug)]
pub struct ImmutableImage<F, A = PotentialDedicatedAllocation<StdMemoryPoolAlloc>> {
    image: UnsafeImage,
    view: UnsafeImageView,
    dimensions: Dimensions,
    memory: A,
    format: F,
    initialized: AtomicBool,
    layout: ImageLayout,
}

impl<F> ImmutableImage<F> {
    /// Builds an uninitialized immutable image.
    pub fn uninitialized<'a, I, M>(
        device: Arc<Device>,
        dimensions: Dimensions,
        format: F,
        mipmaps: M,
        usage: ImageUsage,
        layout: ImageLayout,
        queue_families: I,
    ) -> Result<Arc<ImmutableImage<F>>, ImageCreationError>
    where
        F: FormatDesc,
        I: IntoIterator<Item = QueueFamily<'a>>,
        M: Into<MipmapsCount>,
    {
        let queue_families = queue_families
            .into_iter()
            .map(|f| f.id())
            .collect::<SmallVec<[u32; 4]>>();

        let (image, mem_reqs) = unsafe {
            let sharing = if queue_families.len() >= 2 {
                Sharing::Concurrent(queue_families.iter().cloned())
            } else {
                Sharing::Exclusive
            };

            UnsafeImage::new(
                device.clone(),
                usage,
                format.format(),
                dimensions.to_image_dimensions(),
                1,
                mipmaps,
                sharing,
                false,
                false,
            )?
        };

        let mem = MemoryPool::alloc_from_requirements(
            &Device::standard_pool(&device),
            &mem_reqs,
            AllocLayout::Optimal,
            MappingRequirement::DoNotMap,
            DedicatedAlloc::Image(&image),
            |t| {
                if t.is_device_local() {
                    AllocFromRequirementsFilter::Preferred
                } else {
                    AllocFromRequirementsFilter::Allowed
                }
            },
        )?;
        debug_assert!((mem.offset() % mem_reqs.alignment) == 0);
        unsafe {
            image.bind_memory(mem.memory(), mem.offset())?;
        }

        let view = unsafe {
            UnsafeImageView::raw(
                &image,
                dimensions.to_view_type(),
                0..image.mipmap_levels(),
                0..image.dimensions().array_layers(),
            )?
        };

        let image = Arc::new(ImmutableImage {
            image,
            view,
            memory: mem,
            dimensions,
            format,
            initialized: AtomicBool::new(false),
            layout,
        });

        Ok(image)
    }

    /// Construct an ImmutableImage from the contents of `iter`.
    ///
    /// TODO: Support mipmaps
    #[inline]
    pub fn from_iter<P, I>(
        iter: I,
        dimensions: Dimensions,
        format: F,
        queue: Arc<Queue>,
    ) -> Result<
        (
            Arc<Self>,
            CommandBufferExecFuture<NowFuture, AutoCommandBuffer>,
        ),
        ImageCreationError,
    >
    where
        P: Send + Sync + Clone + 'static,
        F: FormatDesc + AcceptsPixels<P> + 'static + Send + Sync,
        I: ExactSizeIterator<Item = P>,
        Format: AcceptsPixels<P>,
    {
        let source = CpuAccessibleBuffer::from_iter(
            queue.device().clone(),
            BufferUsage::transfer_source(),
            false,
            iter,
        )?;
        ImmutableImage::from_buffer(source, dimensions, format, queue)
    }

    /// Construct an ImmutableImage containing a copy of the data in `source`.
    ///
    /// TODO: Support mipmaps
    pub fn from_buffer<B, P>(
        source: B,
        dimensions: Dimensions,
        format: F,
        queue: Arc<Queue>,
    ) -> Result<
        (
            Arc<Self>,
            CommandBufferExecFuture<NowFuture, AutoCommandBuffer>,
        ),
        ImageCreationError,
    >
    where
        B: BufferAccess + TypedBufferAccess<Content = [P]> + 'static + Clone + Send + Sync,
        P: Send + Sync + Clone + 'static,
        F: FormatDesc + AcceptsPixels<P> + 'static + Send + Sync,
        Format: AcceptsPixels<P>,
    {
        let usage = ImageUsage {
            transfer_destination: true,
            sampled: true,
            ..ImageUsage::none()
        };
        let layout = ImageLayout::ShaderReadOnlyOptimal;

        let buffer = ImmutableImage::uninitialized(
            source.device().clone(),
            dimensions,
            format,
            MipmapsCount::One,
            usage,
            layout,
            source.device().active_queue_families(),
        )?;

        let cb = crate::command_buffer::cb::TextureUploadBuilder::new(queue.clone())?
            .dimensions(dimensions)
            .format(format)
            .build(iter);
        let cb = AutoCommandBufferBuilder::new(source.device().clone(), queue.family())?
            .copy_buffer_to_image_dimensions(
                source,
                init,
                [0, 0, 0],
                dimensions.width_height_depth(),
                0,
                dimensions.array_layers_with_cube(),
                0,
            )
            .unwrap()
            .build()
            .unwrap();

        let future = match cb.execute(queue) {
            Ok(f) => f,
            Err(_) => unreachable!(),
        };

        Ok((buffer, future))
    }
}

impl<F, A> ImmutableImage<F, A> {
    /// Returns the dimensions of the image.
    #[inline]
    pub fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    /// Returns the number of mipmap levels of the image.
    #[inline]
    pub fn mipmap_levels(&self) -> u32 {
        self.image.mipmap_levels()
    }
}

unsafe impl<F, A> ImageAccess for ImmutableImage<F, A>
where
    F: 'static + Send + Sync,
{
    #[inline]
    fn inner(&self) -> ImageInner {
        ImageInner {
            image: &self.image,
            first_layer: 0,
            num_layers: self.image.dimensions().array_layers() as usize,
            first_mipmap_level: 0,
            num_mipmap_levels: self.image.mipmap_levels() as usize,
        }
    }

    #[inline]
    fn initial_layout_requirement(&self) -> ImageLayout {
        self.layout
    }

    #[inline]
    fn final_layout_requirement(&self) -> ImageLayout {
        self.layout
    }

    #[inline]
    fn conflicts_buffer(&self, other: &dyn BufferAccess) -> bool {
        false
    }

    #[inline]
    fn conflicts_image(&self, other: &dyn ImageAccess) -> bool {
        self.conflict_key() == other.conflict_key() // TODO:
    }

    #[inline]
    fn conflict_key(&self) -> u64 {
        self.image.key()
    }

    #[inline]
    fn try_gpu_lock(
        &self,
        exclusive_access: bool,
        expected_layout: ImageLayout,
    ) -> Result<(), AccessError> {
        if expected_layout != self.layout && expected_layout != ImageLayout::Undefined {
            return Err(AccessError::UnexpectedImageLayout {
                requested: expected_layout,
                allowed: self.layout,
            });
        }

        if exclusive_access {
            return Err(AccessError::ExclusiveDenied);
        }

        if !self.initialized.load(Ordering::Relaxed) {
            return Err(AccessError::BufferNotInitialized);
        }

        Ok(())
    }

    #[inline]
    unsafe fn increase_gpu_lock(&self) {}

    #[inline]
    unsafe fn unlock(&self, new_layout: Option<ImageLayout>) {
        debug_assert!(new_layout.is_none());
    }
}

unsafe impl<P, F, A> ImageContent<P> for ImmutableImage<F, A>
where
    F: 'static + Send + Sync,
{
    #[inline]
    fn matches_format(&self) -> bool {
        true // FIXME:
    }
}

unsafe impl<F: 'static, A> ImageViewAccess for ImmutableImage<F, A>
where
    F: 'static + Send + Sync,
{
    #[inline]
    fn parent(&self) -> &dyn ImageAccess {
        self
    }

    #[inline]
    fn dimensions(&self) -> Dimensions {
        self.dimensions
    }

    #[inline]
    fn inner(&self) -> &UnsafeImageView {
        &self.view
    }

    #[inline]
    fn descriptor_set_storage_image_layout(&self) -> ImageLayout {
        self.layout
    }

    #[inline]
    fn descriptor_set_combined_image_sampler_layout(&self) -> ImageLayout {
        self.layout
    }

    #[inline]
    fn descriptor_set_sampled_image_layout(&self) -> ImageLayout {
        self.layout
    }

    #[inline]
    fn descriptor_set_input_attachment_layout(&self) -> ImageLayout {
        self.layout
    }

    #[inline]
    fn identity_swizzle(&self) -> bool {
        true
    }
}

impl<F, A> PartialEq for ImmutableImage<F, A>
where
    F: 'static + Send + Sync,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        ImageAccess::inner(self) == ImageAccess::inner(other)
    }
}

impl<F, A> Eq for ImmutableImage<F, A> where F: 'static + Send + Sync {}

impl<F, A> Hash for ImmutableImage<F, A>
where
    F: 'static + Send + Sync,
{
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        ImageAccess::inner(self).hash(state);
    }
}
