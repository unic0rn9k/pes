#![deny(clippy::all)]

use log::error;
use pixels::{Pixels, SurfaceTexture};
use std::cell::RefCell;
use std::rc::Rc;
//use web_sys;
use lazy_static::*;
use winit::dpi::LogicalSize;
use winit::event::{Event, VirtualKeyCode};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::window::WindowBuilder;
use winit_input_helper::WinitInputHelper;

type Rgba = [u8; 4];

const WIDTH: u32 = 320;
const HEIGHT: u32 = 240;
const SHELL_SPACE: f32 = 20.;
const ENERGY_IN_BRINT_SHELL: &'static [f32] = &[f32::NAN, -13.6, -3.4, -1.5, -0.85, -0.54, -0.38];
const ELECTRON_COLOR: Rgba = [0x00, 0x11, 0x11, 0xff];
const SELECTED_COLOR: Rgba = [0x11, 0x11, 0x22, 0xff];

static mut STATE: u32 = 777;

fn rand() -> u32 {
    unsafe {
        STATE = STATE * 1664525 + 1013904223;
        return STATE >> 24;
    }
}

impl Random for u8 {
    fn random() -> u8 {
        (rand() % 255) as u8
    }
}

trait Random {
    fn random() -> Self;
}

fn random_bytes<const LEN: usize>() -> [u8; LEN] {
    let mut tmp = [0; LEN];
    for n in 0..LEN {
        tmp[n] = u8::random()
    }
    tmp
}

macro_rules! impl_rand {
    ($($t: ty),*) => {
        $(
            impl Random for $t {
                fn random() -> $t {
                    <$t>::from_ne_bytes(random_bytes())
                }
            }
        )*
    };
}

impl_rand!(f32, f64, u32, u64, usize, u16, isize, i8, i16, i32, i64);

impl Random for bool {
    fn random() -> bool {
        rand() % 2 == 0
    }
}

fn render_text(x: usize, y: usize, txt: &str, buffer: &mut [u8]) {
    use font8x8::UnicodeFonts;
    use font8x8::BASIC_FONTS as FONT;

    for (i, c) in txt.chars().enumerate() {
        for (j, row_bits) in FONT.get(c).unwrap().iter().enumerate() {
            for k in 0..8 {
                let bit = *row_bits & 1 << k;
                let point = (x + i * 8 + k + (y + j) * WIDTH as usize) * 4;
                if point + 4 >= buffer.len() {
                    return;
                }

                match bit {
                    0 => buffer[point..point + 4].copy_from_slice(&[0, 0, 0, 255]),
                    _ => buffer[point..point + 4].copy_from_slice(&[255; 4]),
                }
            }
        }
    }
}

macro_rules! render_text {
    ($x: expr, $y: expr, $buffer: expr, $($txt: tt)*) => {
        render_text($x, $y, &format!($($txt)*), $buffer)
    };
}

lazy_static! {
    static ref ORBIT: ([f32; 255], [f32; 255]) = {
        let mut proton_x = [0f32; 255];
        let mut proton_y = [0f32; 255];
        for n in 0..255 {
            proton_x[n] = (n as f32 / 255. * 2. * std::f32::consts::PI).sin() * SHELL_SPACE;
            proton_y[n] = (n as f32 / 255. * 2. * std::f32::consts::PI).cos() * SHELL_SPACE;
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

trait RenderableParticle {
    type UpdateArgs;
    type NewArgs;

    fn update(&mut self, args: Self::UpdateArgs) -> bool;
    fn draw(&self, frame: &mut [u8]);
    fn new(args: Self::NewArgs) -> Self;
    fn x(&self) -> f32;
    fn y(&self) -> f32;
}

impl<T: RenderableParticle<UpdateArgs = U, NewArgs = N>, U, N> RenderableParticle
    for Rc<RefCell<T>>
{
    type UpdateArgs = U;
    type NewArgs = N;

    fn update(&mut self, args: U) -> bool {
        self.as_ref().borrow_mut().update(args)
    }
    fn new(args: N) -> Self {
        Rc::new(T::new(args).into())
    }
    fn draw(&self, frame: &mut [u8]) {
        self.as_ref().borrow().draw(frame)
    }
    fn x(&self) -> f32 {
        self.as_ref().borrow().x()
    }
    fn y(&self) -> f32 {
        self.as_ref().borrow().y()
    }
}

#[derive(Clone, Copy)]
struct Particle {
    x: f32,
    y: f32,
    r: f32,
    dx: f32,
    dy: f32,
    rgba: Rgba,
}

#[derive(Clone)]
struct Electron {
    p: Rc<RefCell<Particle>>,
    shell: u8,
}

impl RenderableParticle for Electron {
    type UpdateArgs = u8;
    type NewArgs = u8;

    fn new(shell: u8) -> Self {
        Self {
            p: Rc::new(Particle::new((16, 16)).into()),
            shell,
        }
    }

    fn update(&mut self, frame: u8) -> bool {
        let x = orbit_x(frame, self.shell as f32);
        let y = orbit_y(frame, self.shell as f32);

        self.p.borrow_mut().update((x, y));
        false
    }

    fn draw(&self, frame: &mut [u8]) {
        self.p.as_ref().borrow().draw(frame);
    }

    fn x(&self) -> f32 {
        self.p.x()
    }
    fn y(&self) -> f32 {
        self.p.y()
    }
}

#[derive(Clone)]
struct Photon {
    p: Particle,
    t: Rc<RefCell<Electron>>,
}

impl RenderableParticle for Photon {
    type UpdateArgs = ();
    type NewArgs = (Rc<RefCell<Electron>>, bool);

    fn new(args: (Rc<RefCell<Electron>>, bool)) -> Self {
        let is_leaving = args.1;
        let t = args.0;

        let mut x = 10.;
        let mut y = 10.;

        let on_x_edge = bool::random();
        if bool::random() {
            if on_x_edge {
                x = WIDTH as f32 - 10.;
            } else {
                y = HEIGHT as f32 - 10.;
            }
        }
        if on_x_edge {
            y = f32::random() % HEIGHT as f32;
        } else {
            x = f32::random() % WIDTH as f32;
        }

        let rp = Particle {
            x,
            y,
            dx: 0.,
            dy: 0.,
            rgba: [1, 1, 0, 1],
            r: 4.0,
        };

        let mut res = if is_leaving {
            Self {
                p: *t.as_ref().borrow().p.as_ref().borrow(),
                t: Rc::new(RefCell::new(Electron {
                    p: Rc::new(rp.into()),
                    shell: 0,
                })),
            }
        } else {
            Self { p: rp, t }
        };

        res.p.rgba = [1, 1, 0, 1];
        res
    }
    fn update(&mut self, _: ()) -> bool {
        self.p.update((self.t.x(), self.t.y()));
        let collided = (self.p.x() - self.t.x()).hypot(self.p.y() - self.t.y()) < 10.;
        if collided {
            self.t.borrow_mut().shell -= 1;
        }
        collided
    }
    fn draw(&self, frame: &mut [u8]) {
        self.p.draw(frame)
    }
    fn x(&self) -> f32 {
        self.p.x()
    }
    fn y(&self) -> f32 {
        self.p.y()
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

    let mut electrons = vec![Rc::new(RefCell::new(Electron::new(2)))];
    let mut photons: Vec<Photon> = vec![];
    let mut frame: u8 = 0;
    let mut selected_electron = 0;

    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame
        if let Event::RedrawRequested(_) = event {
            //input.mouse().map(|(mx, my)| {
            //    //let (mx, my) = pixels.window_pos_to_pixel((mx, my)).unwrap_or((0, 0));

            //    //mouse.draw(pixels.get_frame());
            //});

            electrons.iter_mut().for_each(|e| {
                e.update(frame);
            });
            let mut delete_me = vec![];
            for (n, p) in photons.iter_mut().enumerate() {
                if p.update(()) {
                    delete_me.push(n);
                    p.t.as_ref().borrow_mut().shell -= 1;
                }
            }
            for delete_me in delete_me {
                photons.remove(delete_me);
            }
            frame += 1;

            if pixels.render().is_err() {
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Handle input events
        if input.update(&event) {
            // Close events
            if input.key_pressed(VirtualKeyCode::Space) {
                electrons[selected_electron]
                    .borrow_mut()
                    .p
                    .borrow_mut()
                    .rgba = ELECTRON_COLOR;
                selected_electron += 1;
                if selected_electron >= electrons.len() {
                    selected_electron = 0
                }
                electrons[selected_electron]
                    .borrow_mut()
                    .p
                    .borrow_mut()
                    .rgba = SELECTED_COLOR;
            }
            if input.key_pressed(VirtualKeyCode::J) {
                photons.push(Photon::new((electrons[selected_electron].clone(), false)))
            }
            if input.key_pressed(VirtualKeyCode::K) {
                photons.push(Photon::new((electrons[selected_electron].clone(), true)));
                electrons[selected_electron].borrow_mut().shell += 1;
            }
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window
            if let Some(size) = input.window_resized() {
                pixels.resize_surface(size.width, size.height);
            }

            let frame = pixels.get_frame();
            frame.iter_mut().enumerate().for_each(|(n, p)| {
                let mut p2 = *p;
                p2 -= 4;
                *p = p2 * (p2 < *p) as u8;
                let x = (n / 4) % WIDTH as usize;
                let y = (n / 4 - x) / WIDTH as usize;
                let x = WIDTH as isize / 2 - x as isize;
                let y = HEIGHT as isize / 2 - y as isize;
                let delta = (x as f32).hypot(y as f32) as usize;

                if delta % SHELL_SPACE as usize == 0 && delta < 100 {
                    *p = 70;
                }
            });

            let mut proton = Particle::new((WIDTH as i16 / 2, HEIGHT as i16 / 2));
            proton.rgba = [0xdd, 0xaa, 0x11, 0xff];
            proton.draw(frame);

            electrons.iter().for_each(|e| e.draw(frame));
            photons.iter().for_each(|e| e.draw(frame));

            let ev = ENERGY_IN_BRINT_SHELL
                .get(electrons[selected_electron].as_ref().borrow().shell as usize)
                .unwrap_or(&f32::NAN);
            render_text!(10, 10, frame, "Highlighted electron's energy: {ev} eV");

            // Update internal state and request a redraw
            window.request_redraw();
        }
    });
}

impl RenderableParticle for Particle {
    type NewArgs = (i16, i16);
    type UpdateArgs = (f32, f32);

    fn new(p: (i16, i16)) -> Self {
        Self {
            x: p.0 as f32,
            y: p.1 as f32,
            r: 4.,
            dx: 10.,
            dy: 0.,
            rgba: [0, 1, 1, 255],
        }
    }

    /// Update the `World` internal state; bounce the box around the screen.
    fn update(&mut self, t: (f32, f32)) -> bool {
        if self.x + self.r > WIDTH as f32 || self.x - self.r < 0. {
            self.dx *= -1.;
        }
        if self.y + self.r > HEIGHT as f32 || self.y - self.r < 0. {
            self.dy *= -1.;
        }
        let dx = t.0 - self.x;
        let dy = t.1 - self.y;

        self.dx += dx * 0.06;
        self.dy += dy * 0.06;

        self.dx *= 0.7;
        self.dy *= 0.7;

        self.x += self.dx;
        self.y += self.dy;
        false
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as f32;
            let y = (i as f32 - x) / WIDTH as f32;

            let delta = (x - self.x).hypot(y - self.y);
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

    fn x(&self) -> f32 {
        self.x
    }
    fn y(&self) -> f32 {
        self.y
    }
}
