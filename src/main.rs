#![deny(clippy::all)]

use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::rc::Rc;
//use web_sys;
use font8x8::BASIC_FONTS as FONT;
use lazy_static::*;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const SHELL_SPACE: f32 = 20.;

lazy_static! {
    static ref ORBIT: ([f32; 255], [f32; 255]) = {
        let mut proton_x = [0f32; 255];
        let mut proton_y = [0f32; 255];
        for n in 0..255 {
            proton_x[n] = (n as f32 / 255. * 16. * std::f32::consts::PI).sin() * SHELL_SPACE;
            proton_y[n] = (n as f32 / 255. * 16. * std::f32::consts::PI).cos() * SHELL_SPACE;
        }
        (proton_x, proton_y)
    };
}

fn orbit_x(frame: u8, shell: f32) -> f32 {
    unsafe { WIDTH as f32 / 2. + ORBIT.0.get_unchecked(frame as usize) * shell }
}
fn orbit_y(frame: u8, shell: f32) -> f32 {
    unsafe { HEIGHT as f32 / 2. + ORBIT.1.get_unchecked(frame as usize) * shell }
}

/// Representation of the application state. In this example, a box will bounce around the screen.
struct Particle {
    x: f32,
    y: f32,
    r: f32,
    dx: f32,
    dy: f32,
    rgba: [u8; 4],
}

struct Electron {
    p: Particle,
    shell: u8,
}

impl Electron {
    fn new(shell: u8) -> Self {
        Self {
            p: Particle::new(16, 16),
            shell,
        }
    }

    fn update(&mut self, frame: u8) {
        let x = orbit_x(frame, self.shell as f32);
        let y = orbit_y(frame, self.shell as f32);
        self.p.update(x, y)
    }
}

fn main() {
    #[cfg(target_arch = "wasm32")]
    {
        std::panic::set_hook(Box::new(console_error_panic_hook::hook));
        console_log::init_with_level(log::Level::Trace).expect("error initializing logger");
        wasm_bindgen_futures::spawn_local(run());
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        env_logger::init();
        pollster::block_on(run());
    }
}

async fn run() {
    let event_loop = EventLoop::new();
    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        WindowBuilder::new()
            .with_title("P.E.S.")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .expect("WindowBuilder error")
    };

    let window = Rc::new(window);

    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;
        use winit::platform::web::WindowExtWebSys;

        // Retrieve current width and height dimensions of browser client window
        let get_window_size = || {
            let client_window = web_sys::window().unwrap();
            LogicalSize::new(
                client_window.inner_width().unwrap().as_f64().unwrap(),
                client_window.inner_height().unwrap().as_f64().unwrap(),
            )
        };

        let window = Rc::clone(&window);

        // Initialize winit window with current dimensions of browser client
        window.set_inner_size(get_window_size());

        let client_window = web_sys::window().unwrap();

        // Attach winit canvas to body element
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| doc.body())
            .and_then(|body| {
                body.append_child(&web_sys::Element::from(window.canvas()))
                    .ok()
            })
            .expect("couldn't append canvas to document body");

        // Listen for resize event on browser client. Adjust winit window dimensions
        // on event trigger
        let closure = wasm_bindgen::closure::Closure::wrap(Box::new(move |_e: web_sys::Event| {
            let size = get_window_size();
            window.set_inner_size(size)
        }) as Box<dyn FnMut(_)>);
        client_window
            .add_event_listener_with_callback("resize", closure.as_ref().unchecked_ref())
            .unwrap();
        closure.forget();
    }

    let mut input = WinitInputHelper::new();
    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width, window_size.height, window.as_ref());
        Pixels::new_async(WIDTH, HEIGHT, surface_texture)
            .await
            .expect("Pixels error")
    };

    let mut electrons = [
        Electron::new(1),
        Electron::new(2),
        Electron::new(3),
        Electron::new(4),
    ];
    let mut frame = 0u8;

    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            //input.mouse().map(|(mx, my)| {
            //    //let (mx, my) = pixels.window_pos_to_pixel((mx, my)).unwrap_or((0, 0));

            //    //mouse.draw(pixels.get_frame());
            //});

            electrons.iter_mut().for_each(|e| e.update(frame));
            frame += 1;

            if pixels.render().is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }

            pixels
                .get_frame()
                .iter_mut()
                .enumerate()
                .for_each(|(n, p)| {
                    let mut p2 = *p;
                    p2 -= 4;
                    *p = p2 * (p2 < *p) as u8;
                    let x = (n / 4) % WIDTH as usize;
                    let y = (n / 4 - x) / WIDTH as usize;
                    let x = WIDTH as isize / 2 - x as isize;
                    let y = HEIGHT as isize / 2 - y as isize;
                    let delta = ((x * x + y * y) as f32).sqrt() as usize;

                    if delta % SHELL_SPACE as usize == 0 && delta < 100 {
                        *p = 70;
                    }
                });

            let mut proton = Particle::new(WIDTH as i16 / 2, HEIGHT as i16 / 2);
            proton.rgba = [0xff, 0xaa, 0x00, 0xff];
            proton.draw(pixels.get_frame());

            electrons.iter().for_each(|e| e.p.draw(pixels.get_frame()));

            // Update internal state and request a redraw
            window.request_redraw();
        }
    });
}

impl Particle {
    /// Create a new `World` instance that can draw a moving box.
    fn new(x: i16, y: i16) -> Self {
        Self {
            x: x as f32,
            y: y as f32,
            r: 4.,
            dx: 10.,
            dy: 0.,
            rgba: [0, 1, 1, 255],
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self, tx: f32, ty: f32) {
        if self.x + self.r > WIDTH as f32 || self.x - self.r < 0. {
            self.dx *= -1.;
        }
        if self.y + self.r > HEIGHT as f32 || self.y - self.r < 0. {
            self.dy *= -1.;
        }
        let dx = tx - self.x;
        let dy = ty - self.y;

        self.dx += dx * 0.05;
        self.dy += dy * 0.05;

        self.dx *= 0.8;
        self.dy *= 0.8;

        self.x += self.dx;
        self.y += self.dy;
    }

    /// Draw the `World` state to the frame buffer.
    ///
    /// Assumes the default texture format: `wgpu::TextureFormat::Rgba8UnormSrgb`
    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as f32;
            let y = (i as f32 - x) / WIDTH as f32;

            let delta = ((x - self.x).powi(2) + (y - self.y).powi(2)).sqrt();
            let inside_the_box = delta < self.r;

            let mut rgba = self.rgba;
            if inside_the_box {
                for n in 0..4 {
                    *unsafe { rgba.get_unchecked_mut(n) } *=
                        ((self.r - delta) / self.r * 300.) as u8
                }
            } else {
                continue;
            };

            pixel.copy_from_slice(&rgba);
        }
    }
}
