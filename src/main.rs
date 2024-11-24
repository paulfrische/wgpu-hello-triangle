use std::{
    borrow::Cow,
    sync::{mpsc, Arc},
};

use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowAttributes, WindowId},
};

struct EventHandler {
    event_sender: mpsc::Sender<Event>,
}

enum Event {
    WindowCreated(Window),
    WindowClose,
    RedrawRequested,
}

struct State<'state> {
    event_receiver: mpsc::Receiver<Event>,
    window: Option<Arc<Window>>,
    gfx: Option<Gfx<'state>>,
}

struct Gfx<'gfx> {
    surface: wgpu::Surface<'gfx>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
}

impl<'gfx> Gfx<'gfx> {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        let size = window.as_ref().inner_size();

        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone())?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("no adapter found!"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(), // maybe source of some trouble
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            )
            .await?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(swapchain_format.into())],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .ok_or_else(|| anyhow::anyhow!("failed to create config!"))?;
        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            pipeline,
        })
    }
}

impl<'state> State<'state> {
    pub async fn new(receiver: mpsc::Receiver<Event>) -> State<'state> {
        Self {
            event_receiver: receiver,
            window: None,
            gfx: None,
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        match self.event_receiver.recv()? {
            Event::WindowCreated(window) => self.window = Some(Arc::new(window)),
            _ => Err(anyhow::anyhow!("unexpected event"))?,
        };

        self.gfx = Some(Gfx::new(self.window.clone().unwrap()).await?);

        loop {
            match self.event_receiver.recv()? {
                Event::RedrawRequested => {
                    let gfx = self.gfx.as_ref().unwrap();

                    let frame = gfx.surface.get_current_texture()?;
                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = gfx
                        .device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                        rpass.set_pipeline(&gfx.pipeline);
                        rpass.draw(0..3, 0..1);
                    }

                    gfx.queue.submit(Some(encoder.finish()));
                    frame.present();
                }
                Event::WindowClose => break,
                _ => {}
            }
        }
        Ok(())
    }
}

impl ApplicationHandler for EventHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = event_loop
            .create_window(
                WindowAttributes::default()
                    .with_resizable(false)
                    .with_inner_size(PhysicalSize::<u32>::from((1280, 720))),
            )
            .unwrap();

        self.event_sender
            .send(Event::WindowCreated(window))
            .unwrap();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.event_sender.send(Event::WindowClose).unwrap();
                event_loop.exit();
            }

            WindowEvent::RedrawRequested => {
                self.event_sender.send(Event::RedrawRequested).unwrap();
            }

            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let (tx, rx) = mpsc::channel();

    let mut app = EventHandler { event_sender: tx };
    tokio::spawn(async move {
        let mut state = State::new(rx).await;
        state.run().await.unwrap();
    });

    event_loop.run_app(&mut app)?;

    Ok(())
}
