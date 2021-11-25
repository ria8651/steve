use std::sync::Arc;
use vulkano::buffer::{BufferUsage, CpuAccessibleBuffer, TypedBufferAccess};
use vulkano::command_buffer::{AutoCommandBufferBuilder, CommandBufferUsage, SubpassContents};
use vulkano::device::physical::{PhysicalDevice, PhysicalDeviceType};
use vulkano::device::{Device, DeviceExtensions, Features, Queue};
use vulkano::format::Format;
use vulkano::image::view::ImageView;
use vulkano::image::{ImageUsage, SwapchainImage, attachment::AttachmentImage};
use vulkano::instance::Instance;
use vulkano::pipeline::viewport::Viewport;
use vulkano::pipeline::{GraphicsPipeline, vertex::BuffersDefinition};
use vulkano::render_pass::{Framebuffer, FramebufferAbstract, RenderPass, Subpass};
use vulkano::swapchain;
use vulkano::swapchain::{AcquireError, Surface, Swapchain, SwapchainCreationError};
use vulkano::sync;
use vulkano::sync::{FlushError, GpuFuture};
use vulkano::Version;
use vulkano_win::VkSurfaceBuild;
use winit::event::{Event, KeyboardInput, WindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::{Window, WindowBuilder};

pub struct RenderEngine {
    device: Arc<Device>,
    queue: Arc<Queue>,

    event_loop: EventLoop<()>,
    surface: Arc<Surface<Window>>,

    swapchain: Arc<Swapchain<Window>>,
    framebuffers: Vec<Arc<dyn FramebufferAbstract>>,

    render_pass: Arc<RenderPass>,
    pipeline: Arc<GraphicsPipeline>,

    previous_frame_end: Option<Box<dyn GpuFuture>>,
    recreate_swapchain: bool,

    vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>>,
    normal_buffer: Arc<CpuAccessibleBuffer<[Normal]>>,
    vs: vs::Shader,
    fs: fs::Shader,
}

impl RenderEngine {
    pub fn init() -> RenderEngine {
        let required_extensions = vulkano_win::required_extensions();

        let instance = Instance::new(None, Version::V1_1, &required_extensions, None).unwrap();

        let event_loop = EventLoop::new();
        let surface = WindowBuilder::new()
            .build_vk_surface(&event_loop, instance.clone())
            .unwrap();

        let device_extensions = DeviceExtensions {
            khr_swapchain: true,
            ..DeviceExtensions::none()
        };

        let (physical_device, queue_family) = PhysicalDevice::enumerate(&instance)
            .filter(|&p| p.supported_extensions().is_superset_of(&device_extensions))
            .filter_map(|p| {
                p.queue_families()
                    .find(|&q| q.supports_graphics() && surface.is_supported(q).unwrap_or(false))
                    .map(|q| (p, q))
            })
            .min_by_key(|(p, _)| match p.properties().device_type {
                PhysicalDeviceType::DiscreteGpu => 0,
                PhysicalDeviceType::IntegratedGpu => 1,
                PhysicalDeviceType::VirtualGpu => 2,
                PhysicalDeviceType::Cpu => 3,
                PhysicalDeviceType::Other => 4,
            })
            .unwrap();

        println!(
            "Using device: {} (type: {:?})",
            physical_device.properties().device_name,
            physical_device.properties().device_type,
        );

        let (device, mut queues) = Device::new(
            physical_device,
            &Features::none(),
            &physical_device
                .required_extensions()
                .union(&device_extensions),
            [(queue_family, 0.5)].iter().cloned(),
        )
        .unwrap();

        let queue = queues.next().unwrap();

        let (swapchain, images) = {
            let caps = surface.capabilities(physical_device).unwrap();

            let composite_alpha = caps.supported_composite_alpha.iter().next().unwrap();

            let format = caps.supported_formats[0].0;

            let dimensions: [u32; 2] = surface.window().inner_size().into();

            Swapchain::start(device.clone(), surface.clone())
                .num_images(caps.min_image_count)
                .format(format)
                .dimensions(dimensions)
                .usage(ImageUsage::color_attachment())
                .sharing_mode(&queue)
                .composite_alpha(composite_alpha)
                .build()
                .unwrap()
        };

        let render_pass = Arc::new(
            vulkano::single_pass_renderpass!(device.clone(),
                attachments: {
                    color: {
                        load: Clear,
                        store: Store,
                        format: swapchain.format(),
                        samples: 1,
                    },
                    depth: {
                        load: Clear,
                        store: DontCare,
                        format: Format::D16_UNORM,
                        samples: 1,
                    }
                },
                pass: {
                    color: [color],
                    depth_stencil: {depth}
                }
            )
            .unwrap(),
        );

        let vertex_buffer: Arc<CpuAccessibleBuffer<[Vertex]>> = CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage::all(),
            false,
            [
                Vertex {
                    position: [-1.0, -1.0, 0.5],
                },
                Vertex {
                    position: [-1.0, 1.0, 1.0],
                },
                Vertex {
                    position: [1.0, 1.0, 0.0],
                },
                Vertex {
                    position: [1.0, 1.0, 0.5],
                },
                Vertex {
                    position: [-1.0, -1.0, 0.0],
                },
                Vertex {
                    position: [1.0, -1.0, 1.0],
                },
            ]
            .iter()
            .cloned(),
        )
        .unwrap();

        let normal_buffer: Arc<CpuAccessibleBuffer<[Normal]>> = CpuAccessibleBuffer::from_iter(
            device.clone(),
            BufferUsage::all(),
            false,
            [
                Normal {
                    normal: [1.0, 0.0, 0.0],
                },
                Normal {
                    normal: [0.0, 1.0, 0.0],
                },
                Normal {
                    normal: [0.0, 0.0, 1.0],
                },
                Normal {
                    normal: [0.0, 0.0, 1.0],
                },
                Normal {
                    normal: [1.0, 0.0, 0.0],
                },
                Normal {
                    normal: [0.0, 1.0, 0.0],
                },
            ]
            .iter()
            .cloned(),
        )
        .unwrap();

        let vs = vs::Shader::load(device.clone()).unwrap();
        let fs = fs::Shader::load(device.clone()).unwrap();

        let (pipeline, framebuffers) =
            window_size_dependent_setup(device.clone(), &vs, &fs, &images, render_pass.clone());

        let previous_frame_end = Some(sync::now(device.clone()).boxed());

        RenderEngine {
            device: device,
            queue: queue,

            event_loop: event_loop,
            surface: surface,

            swapchain: swapchain,
            framebuffers: framebuffers,

            render_pass: render_pass,
            pipeline: pipeline,

            previous_frame_end: previous_frame_end,
            recreate_swapchain: false,

            vertex_buffer: vertex_buffer,
            normal_buffer: normal_buffer,
            vs: vs,
            fs: fs,
        }
    }

    pub fn game_loop(mut render_engine: RenderEngine) {
        render_engine.draw_frame();

        render_engine.event_loop.run(move |event, _, control_flow| {
            use winit::event::{ElementState, VirtualKeyCode};
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::CloseRequested => {
                        *control_flow = ControlFlow::Exit;
                    }
                    WindowEvent::Resized(_) => {
                        render_engine.recreate_swapchain = true;
                    }
                    WindowEvent::KeyboardInput {
                        input:
                            KeyboardInput {
                                virtual_keycode: Some(virtual_code),
                                state: ElementState::Pressed,
                                ..
                            },
                        ..
                    } => match virtual_code {
                        VirtualKeyCode::Q => {
                            *control_flow = ControlFlow::Exit;
                        }
                        _ => (),
                    },
                    _ => (),
                },
                Event::RedrawEventsCleared => {}
                _ => (),
            }
        });
    }

    pub fn draw_frame(&mut self) {
        self.previous_frame_end.as_mut().unwrap().cleanup_finished();

        if self.recreate_swapchain {
            let dimensions: [u32; 2] = self.surface.window().inner_size().into();
            let (new_swapchain, new_images) =
                match self.swapchain.recreate().dimensions(dimensions).build() {
                    Ok(r) => r,
                    Err(SwapchainCreationError::UnsupportedDimensions) => return,
                    Err(e) => panic!("Failed to recreate swapchain: {:?}", e),
                };

            self.swapchain = new_swapchain;
            let (new_pipeline, new_framebuffers) = window_size_dependent_setup(
                self.device.clone(),
                &self.vs,
                &self.fs,
                &new_images,
                self.render_pass.clone(),
            );
            self.pipeline = new_pipeline;
            self.framebuffers = new_framebuffers;
            self.recreate_swapchain = false;
        }

        let (image_num, suboptimal, acquire_future) =
            match swapchain::acquire_next_image(self.swapchain.clone(), None) {
                Ok(r) => r,
                Err(AcquireError::OutOfDate) => {
                    self.recreate_swapchain = true;
                    return;
                }
                Err(e) => panic!("Failed to acquire next image: {:?}", e),
            };

        if suboptimal {
            self.recreate_swapchain = true;
        }

        let clear_values = vec![[0.3, 0.7, 0.2, 1.0].into(), 1.0.into()];

        let mut builder = AutoCommandBufferBuilder::primary(
            self.device.clone(),
            self.queue.family(),
            CommandBufferUsage::OneTimeSubmit,
        )
        .unwrap();

        builder
            .begin_render_pass(
                self.framebuffers[image_num].clone(),
                SubpassContents::Inline,
                clear_values,
            )
            .unwrap()
            .bind_pipeline_graphics(self.pipeline.clone())
            .bind_vertex_buffers(0, (self.vertex_buffer.clone(), self.normal_buffer.clone()))
            .draw(self.vertex_buffer.len() as u32, 1, 0, 0)
            .unwrap()
            .end_render_pass()
            .unwrap();

        let command_buffer = builder.build().unwrap();

        let future = self
            .previous_frame_end
            .take()
            .unwrap()
            .join(acquire_future)
            .then_execute(self.queue.clone(), command_buffer)
            .unwrap()
            .then_swapchain_present(self.queue.clone(), self.swapchain.clone(), image_num)
            .then_signal_fence_and_flush();

        match future {
            Ok(future) => {
                self.previous_frame_end = Some(future.boxed());
            }
            Err(FlushError::OutOfDate) => {
                self.recreate_swapchain = true;
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
            Err(e) => {
                println!("Failed to flush future: {:?}", e);
                self.previous_frame_end = Some(sync::now(self.device.clone()).boxed());
            }
        }
    }
}

/// This method is called once during initialization, then again whenever the window is resized
fn window_size_dependent_setup(
    device: Arc<Device>,
    vs: &vs::Shader,
    fs: &fs::Shader,
    images: &[Arc<SwapchainImage<Window>>],
    render_pass: Arc<RenderPass>,
) -> (Arc<GraphicsPipeline>, Vec<Arc<dyn FramebufferAbstract>>) {
    let dimensions = images[0].dimensions();

    let depth_buffer = ImageView::new(
        AttachmentImage::transient(device.clone(), dimensions, Format::D16_UNORM).unwrap(),
    )
    .unwrap();

    let framebuffers = images
        .iter()
        .map(|image| {
            let view = ImageView::new(image.clone()).unwrap();
            Arc::new(
                Framebuffer::start(render_pass.clone())
                    .add(view)
                    .unwrap()
                    .add(depth_buffer.clone())
                    .unwrap()
                    .build()
                    .unwrap(),
            ) as Arc<dyn FramebufferAbstract>
        })
        .collect::<Vec<_>>();

    // In the triangle example we use a dynamic viewport, as its a simple example.
    // However in the teapot example, we recreate the pipelines with a hardcoded viewport instead.
    // This allows the driver to optimize things, at the cost of slower window resizes.
    // https://computergraphics.stackexchange.com/questions/5742/vulkan-best-way-of-updating-pipeline-viewport
    let pipeline = Arc::new(
        GraphicsPipeline::start()
            .vertex_input(
                BuffersDefinition::new()
                    .vertex::<Vertex>()
                    .vertex::<Normal>(),
            )
            .vertex_shader(vs.main_entry_point(), ())
            .triangle_list()
            .viewports_dynamic_scissors_irrelevant(1)
            .viewports([Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0..1.0,
            }])
            .fragment_shader(fs.main_entry_point(), ())
            .depth_stencil_simple_depth()
            .render_pass(Subpass::from(render_pass.clone(), 0).unwrap())
            .build(device.clone())
            .unwrap(),
    );

    (pipeline, framebuffers)
}

#[derive(Default, Debug, Clone)]
struct Vertex {
    position: [f32; 3],
}

vulkano::impl_vertex!(Vertex, position);

#[derive(Default, Copy, Clone)]
pub struct Normal {
    normal: [f32; 3],
}

vulkano::impl_vertex!(Normal, normal);

mod vs {
    vulkano_shaders::shader! {
        ty: "vertex",
        src: "
				#version 450

				layout(location = 0) in vec3 position;
				layout(location = 1) in vec3 normal;

				layout(location = 0) out vec3 v_normal;
                
				void main() {
					gl_Position = vec4(position, 1.0);
                    v_normal = normal;
				}
			"
    }
}

mod fs {
    vulkano_shaders::shader! {
        ty: "fragment",
        src: "
				#version 450

				layout(location = 0) in vec3 v_normal;
				layout(location = 0) out vec4 f_color;

				void main() {
					f_color = vec4(v_normal, 0.0);
				}
			"
    }
}
