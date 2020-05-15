use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};
use crate::{device::Device, sync::Fence, OomError, SafeDeref};

pub struct FenceFuture<T> {
    output: Arc<Mutex<Option<T>>>,
    fence: Fence,
}

impl<T> FenceFuture<T> {
    pub fn new(fence: Fence, output: T) -> Self {
        Self {
            output: Arc::new(Mutex::new(Some(output))),
            fence,
        }
    }
}

impl<T> Future for FenceFuture<T> {
    type Output = Result<T, OomError>;

    fn poll(self: Pin<&mut FenceFuture<T>>, ctx: &mut Context) -> Poll<Self::Output> {
        match self.fence.ready() {
            Ok(ready) => match ready {
                true => return Poll::Ready(Ok(self.output.lock().unwrap().take().unwrap())),
                false => {
                    ctx.waker().wake_by_ref();
                    return Poll::Pending;
                }
            },
            Err(e) => return Poll::Ready(Err(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fmt::{Debug, Display},
        sync::Arc,
        boxed::Box,
    };
    use vulkano::{
        device::{Device, DeviceCreationError},
        instance::{Instance, InstanceCreationError, InstanceExtensions},
        sync::Fence,
        OomError,
    };

    #[derive(Debug)]
    enum Error {
        Instance(InstanceCreationError),
        Device(DeviceCreationError),
        Oom(OomError),
        NoPhysicalDeviceFound,
        NoQueueFamilyFound,
    }

    impl From<DeviceCreationError> for Error {
        fn from(v: DeviceCreationError) -> Self {
            Error::Device(v)
        }
    }

    impl From<InstanceCreationError> for Error {
        fn from(v: InstanceCreationError) -> Self {
            Error::Instance(v)
        }
    }

    impl From<OomError> for Error {
        fn from(v: OomError) -> Self {
            Error::Oom(v)
        }
    }

    impl Display for Error {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Error::Instance(e) => write!(f, "Instance creation error: {}", e),
                Error::Device(e) => write!(f, "Device creation error: {}", e),
                Error::Oom(e) => write!(f, "Out of memory error, possibly catchall: {}", e),
                Error::NoPhysicalDeviceFound => write!(f, "No physical device found!"),
                Error::NoQueueFamilyFound => write!(f, "No queue families found!"),
            }
        }
    }

    async fn make_fence() -> Result<(), Error> {
        let layers: Vec<&str> = vec![];
        let instance_extensions = InstanceExtensions {
            ..InstanceExtensions::none()
        };
        let instance = Instance::new(None, &instance_extensions, layers).map_err(Error::from)?;
        let phys = vulkano::instance::PhysicalDevice::enumerate(&instance)
            .next()
            .ok_or(Error::NoPhysicalDeviceFound)?;
        let requested_features = vulkano::device::Features::none();
        let extensions = vulkano::device::DeviceExtensions::none();
        let queue_family = phys
            .queue_families()
            .next()
            .ok_or(Error::NoQueueFamilyFound)?;
        let (device, queues_iter) = Device::new(
            phys,
            &requested_features,
            &extensions,
            Some((queue_family, 1.0)),
        )
        .map_err(Error::from)?;
        let fence = vulkano::sync::Fence::alloc_signaled(device.clone()).map_err(Error::from)?;
        FenceFuture::new(fence, ())
            .await
            .map_err(Error::from)
    }

    #[tokio::test]
    async fn async_fence() {
        let fence_future = make_fence().await;
    }
}
