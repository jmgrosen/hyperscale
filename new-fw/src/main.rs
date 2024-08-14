#![no_std]
#![no_main]

extern crate alloc;

use core::cell::RefCell;
use core::iter;
use core::slice;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use critical_section::Mutex;
use embedded_graphics::pixelcolor::raw::RawU16;
use embedded_graphics::prelude::{DrawTarget, Point, Size};
use embedded_graphics::primitives::Rectangle;
use embedded_hal_1::digital::InputPin;
use esp_backtrace as _;
use esp_println::println;
use hal::peripherals::TIMG0;
use hal::spi::master::Spi;
use hal::systimer::SystemTimer;
use hal::timer::Timer0;
use hal::Timer;
use hal::{
    clock::{ClockControl, CpuClock}, dma::DmaPriority, gdma::Gdma, gpio::NO_PIN, peripherals::Peripherals,
    prelude::*, timer::TimerGroup, Delay, Rtc, IO,
    gpio::{Input, PullUp, Gpio1, Gpio2, Gpio3, Gpio10, self}, interrupt, peripherals,
    i2c::I2C,
    psram,
};
use hal::spi::master::prelude::*;

use rotary_encoder_hal::{Direction, Rotary, DefaultPhase};
use nau7802::Nau7802;
use nb::block;
use heapless::spsc::{Queue, Producer};
use debouncr::{DebouncerStateful, Edge, Repeat6, debounce_stateful_6};

use slint::platform::software_renderer::{MinimalSoftwareWindow, Rgb565Pixel, TargetPixel, PremultipliedRgbaColor};
use slint::platform::{software_renderer as renderer, Platform, WindowEvent, Key};
use slint::{Model, PhysicalSize};

use t_display_s3_amoled::rm67162::dma::RM67162Dma;
use t_display_s3_amoled::rm67162::Orientation;

#[global_allocator]
static ALLOCATOR: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

/*
fn init_heap() {    
    const HEAP_SIZE: usize = 80 * 1024;
    static mut HEAP: MaybeUninit<[u8; HEAP_SIZE]> = MaybeUninit::uninit();

    unsafe {
        ALLOCATOR.init(HEAP.as_mut_ptr() as *mut u8, HEAP_SIZE);
    }
}
 */

fn init_psram_heap() {
    unsafe {
        ALLOCATOR.init(psram::psram_vaddr_start() as *mut u8, psram::PSRAM_BYTES);
    }
}

enum ButtonEvent {
    Press,
    Release,
    LongPress,
    LongRelease,
}

struct Button<P> {
    pin: P,
    debouncer: DebouncerStateful<u8, Repeat6>,
    held_for: u8,
}

const LONG_PRESS_TICKS: u8 = 100;

impl<P: InputPin> Button<P> {
    fn new(pin: P) -> Button<P> {
        Button { pin, debouncer: debounce_stateful_6(false), held_for: 0 }
    }

    fn update(&mut self) -> Option<ButtonEvent> {
        // logical low is logical high (pulled up)
        match self.debouncer.update(self.pin.is_low().unwrap()) {
            Some(Edge::Rising) => {
                self.held_for = 0;
                Some(ButtonEvent::Press)
            },
            Some(Edge::Falling) =>
                Some(if self.held_for > LONG_PRESS_TICKS {
                    ButtonEvent::LongRelease
                } else {
                    ButtonEvent::Release
                }),
            None if self.debouncer.is_high() => {
                self.held_for = self.held_for.saturating_add(1);
                if self.held_for > LONG_PRESS_TICKS {
                    Some(ButtonEvent::LongPress)
                } else {
                    None
                }
            },
            _ =>
                None,
        }
    }
}

enum Event {
    WheelCW,
    WheelCCW,
    WheelButton(ButtonEvent),
    BackButton(ButtonEvent),
}

struct InterruptResources {
    encoder: Rotary<Gpio1<Input<PullUp>>, Gpio2<Input<PullUp>>, DefaultPhase>,
    producer: Producer<'static, Event, 16>,
    wheel_button: Button<Gpio3<Input<PullUp>>>,
    back_button: Button<Gpio10<Input<PullUp>>>,
    timer: Timer<Timer0<TIMG0>>,
}

static INTERRUPT_RESOURCES: Mutex<RefCell<Option<InterruptResources>>> = Mutex::new(RefCell::new(None));

#[ram]
#[interrupt]
fn GPIO() {
    critical_section::with(|cs| {
        let mut borrowed_resources = INTERRUPT_RESOURCES.borrow_ref_mut(cs);
        let resources = borrowed_resources.as_mut().unwrap();
        match resources.encoder.update().unwrap() {
            Direction::Clockwise => { let _ = resources.producer.enqueue(Event::WheelCW); },
            Direction::CounterClockwise => { let _ = resources.producer.enqueue(Event::WheelCCW); },
            Direction::None => { },
        }
        let (pin_a, pin_b) = resources.encoder.pins();
        pin_a.clear_interrupt();
        pin_b.clear_interrupt();
    });
}

const TIMER_PERIOD_MS: u32 = 5;

#[ram]
#[interrupt]
fn TG0_T0_LEVEL() {
    critical_section::with(|cs| {
        let mut borrowed_resources = INTERRUPT_RESOURCES.borrow_ref_mut(cs);
        let resources = borrowed_resources.as_mut().unwrap();

        if let Some(event) = resources.wheel_button.update() {
            let _ = resources.producer.enqueue(Event::WheelButton(event));
        }
        if let Some(event) = resources.back_button.update() {
            let _ = resources.producer.enqueue(Event::BackButton(event));
        }

        if resources.timer.is_interrupt_set() {
            resources.timer.clear_interrupt();
            resources.timer.start(TIMER_PERIOD_MS.millis());
        }
    });
}

const ONE_KG: f32 = 1.0 / 674500.0;

slint::include_modules!();

struct Backend {
    window: Rc<renderer::MinimalSoftwareWindow>,
}

impl Platform for Backend {
    fn create_window_adapter(
        &self,
    ) -> Result<alloc::rc::Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        // Since on MCUs, there can be only one window, just return a clone of self.window.
        // We'll also use the same window in the event loop.
        Ok(self.window.clone())
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(
            SystemTimer::now() * 1_000 / SystemTimer::TICKS_PER_SECOND,
        )
    }

    // fn run_event_loop(&self) -> Result<(), slint::PlatformError>
    fn debug_log(&self, arguments: core::fmt::Arguments) {
        println!("Slint: {:?}", arguments);
    }
}

struct DisplayWrapper<'a, CS> {
    display: &'a mut RM67162Dma<'a, CS>,
    line_buffer: &'a mut [Rgb565Pixel; 536],
}

impl<CS> renderer::LineBufferProvider for &mut DisplayWrapper<'_, CS>
where
    CS: embedded_hal_1::digital::OutputPin,
{
    type TargetPixel = Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        render_fn(&mut self.line_buffer[range.clone()]);

        let _ = self.display.fill_contiguous(
            &Rectangle::new(
                Point::new(range.start as _, line as _),
                Size::new(range.len() as _, 1),
            ),
            self.line_buffer[range.clone()]
                .iter()
                .map(|p| RawU16::new(p.0).into()),
        );
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Default)]
struct Rgb565PixelFlipped(u16);

// lazy implementation
impl TargetPixel for Rgb565PixelFlipped {
    fn blend(&mut self, color: PremultipliedRgbaColor) {
        let mut pix = Rgb565Pixel((self.0 << 8) | (self.0 >> 8));
        pix.blend(color);
        self.0 = (pix.0 << 8) | (pix.0 >> 8);
    }

    fn from_rgb(r: u8, g: u8, b: u8) -> Rgb565PixelFlipped {
        let pix = Rgb565Pixel::from_rgb(r, g, b);
        Rgb565PixelFlipped((pix.0 << 8) | (pix.0 >> 8))
    }
}

fn cast_pixel_buffer(b: &[Rgb565PixelFlipped]) -> &[u8] {
    unsafe { slice::from_raw_parts(b.as_ptr() as *const u8, b.len() * 2) }
}

struct FramebufLiner {
    framebuf: [Rgb565PixelFlipped; 536 * 240],
}

impl renderer::LineBufferProvider for &mut FramebufLiner {
    type TargetPixel = Rgb565PixelFlipped;

    fn process_line(&mut self, line: usize, range: core::ops::Range<usize>, render_fn: impl FnOnce(&mut [Self::TargetPixel])) {
        render_fn(&mut self.framebuf[(line * 536 + range.start)..(line * 536 + range.end)]);
    }
}

struct ScaleState {
    zero: i32,
        val: i32,
}

impl ScaleState {
    fn new(val: i32) -> ScaleState {
            ScaleState { zero: val, val }
    }

    fn update(&mut self, val: i32) {
            self.val = val;
        }

    fn rezero(&mut self) {
            self.zero = self.val;
        }

    fn in_kg(&self) -> f32 {
            ((self.val - self.zero) as f32) * ONE_KG
        }
}

fn ingredient(name: &str, amount: f32) -> Ingredient {
    Ingredient { name: name.into(), amount }
}

fn vegan_choux() -> Recipe {
    Recipe {
        name: "Vegan Choux".into(),
        ingredients: [
            ingredient("water", 0.06),
            ingredient("soy milk", 0.06),
            ingredient("vanilla extract", 0.005),
            ingredient("sugar", 0.006),
            ingredient("vegan butter", 0.028),
            ingredient("all-purpose flour", 0.065),
            ingredient("Just Egg", 0.125),
            ingredient("soy milk", 0.030),
        ].into(),
    }
}

fn vegan_creme_pat() -> Recipe {
    Recipe {
        name: "Vegan Creme Pat".into(),
        ingredients: [
            ingredient("soy milk", 0.243),
            ingredient("vanilla extract", 0.010),
            ingredient("salt", 0.001),
            ingredient("corn starch", 0.016),
            ingredient("sugar", 0.050),
            ingredient("Just Egg", 0.083),
            ingredient("vegan butter", 0.042),
        ].into(),
    }
}

fn choux() -> Recipe {
    Recipe {
        name: "Choux".into(),
        ingredients: [
            ingredient("water", 0.235),
            ingredient("butter", 0.084),
            ingredient("sugar", 0.008),
            ingredient("salt", 0.002),
            ingredient("all-purpose flour", 0.128),
            ingredient("eggs", 0.200),
        ].into(),
    }
}

fn creme_pat() -> Recipe {
    Recipe {
        name: "Creme Pat".into(),
        ingredients: [
            ingredient("sugar", 0.115),
            ingredient("corn starch", 0.030),
            ingredient("salt", 0.001),
            ingredient("egg yolks", 0.070),
            ingredient("butter", 0.030),
        ].into(),
    }
}

fn progress_for_recipe(recipe: &Recipe) -> RecipeProgress {
    RecipeProgress {
        scale_factor: 1.0,
        ingredient_progresses:
            iter::repeat(IngredientProgress { done: false, amount: 0.0 })
            .take(recipe.ingredients.row_count())
            .collect::<Vec<_>>()[..]
            .into(),
    }
}

#[hal::entry]
fn main() -> ! {
    // init_heap();
    println!("main!");
    let peripherals = Peripherals::take();
    psram::init_psram(peripherals.PSRAM);
    init_psram_heap();
    println!("initted psram");
    let system = peripherals.SYSTEM.split();
    let clocks = ClockControl::configure(system.clock_control, CpuClock::Clock160MHz).freeze();

    // Disable the RTC and TIMG watchdog timers
    let mut rtc = Rtc::new(peripherals.LPWR);
    let timer_group0 = TimerGroup::new(
        peripherals.TIMG0,
        &clocks,
    );
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(
        peripherals.TIMG1,
        &clocks,
    );
    let mut wdt1 = timer_group1.wdt;
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();
    println!("Hello board!");

    // Set GPIO4 as an output, and set its state high initially.
    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    // let mut led = io.pins.gpio38.into_push_pull_output();
    // let _button = io.pins.gpio21.into_pull_down_input();

    // led.set_high().unwrap();

    // Initialize the Delay peripheral, and use it to toggle the LED state in a
    // loop.
    let mut delay = Delay::new(&clocks);

    // set up rotary encoder
    let mut pin_a = io.pins.gpio1.into_pull_up_input();
    pin_a.listen(gpio::Event::AnyEdge);
    let mut pin_b = io.pins.gpio2.into_pull_up_input();
    pin_b.listen(gpio::Event::AnyEdge);
    let rotary = Rotary::new(pin_a, pin_b);
    let event_queue: &'static mut Queue<Event, 16> = {
        static mut Q: Queue<Event, 16> = Queue::new();
        unsafe { &mut Q }
    };
    let (event_producer, mut event_consumer) = event_queue.split();

    let encoder_button = io.pins.gpio3.into_pull_up_input();
    let back_button = io.pins.gpio10.into_pull_up_input();

    let mut timer00 = timer_group0.timer0;
    timer00.start(TIMER_PERIOD_MS.millis());
    timer00.listen();
    
    critical_section::with(|cs| {
        INTERRUPT_RESOURCES.borrow_ref_mut(cs).replace(InterruptResources {
            encoder: rotary,
            producer: event_producer,
            wheel_button: Button::new(encoder_button),
            back_button: Button::new(back_button),
            timer: timer00,
        });
    });

    interrupt::enable(peripherals::Interrupt::GPIO, interrupt::Priority::Priority2).unwrap();
    interrupt::enable(peripherals::Interrupt::TG0_T0_LEVEL, interrupt::Priority::Priority2).unwrap();

    let i2c = I2C::new(peripherals.I2C0, io.pins.gpio43, io.pins.gpio44, 100u32.kHz(), &clocks);
    let mut scale_adc = Nau7802::new(i2c, &mut delay).unwrap();
    let scale = Rc::new(RefCell::new(ScaleState::new(block!(scale_adc.read()).unwrap())));

    println!("init display");

    let sclk = io.pins.gpio47;
    let rst = io.pins.gpio17;
    let cs = io.pins.gpio6;

    let d0 = io.pins.gpio18;
    let d1 = io.pins.gpio7;
    let d2 = io.pins.gpio48;
    let d3 = io.pins.gpio5;

    let mut cs = cs.into_push_pull_output();
    cs.set_high().unwrap();
    

    let mut rst = rst.into_push_pull_output();

    let dma = Gdma::new(peripherals.DMA);
    let dma_channel = dma.channel0;

    // Descriptors should be sized as (BUFFERSIZE / 4092) * 3
    let mut descriptors = [0u32; 12];
    // let spi = Spi::new_half_duplex(
    //     peripherals.SPI2, // use spi2 host
    //     Some(sclk),
    //     Some(d0),
    //     Some(d1),
    //     Some(d2),
    //     Some(d3),
    //     NO_PIN,
    //     75_u32.MHz(), // max 75MHz
    //     hal::spi::SpiMode::Mode0,
    //     &clocks,
    // )
    let spi = Spi::new_half_duplex(
        peripherals.SPI2, // use spi2 host
        80_u32.MHz(), // max 75MHz
        hal::spi::SpiMode::Mode0,
        &clocks,
    )
    .with_pins(
        Some(sclk),
        Some(d0),
        Some(d1),
        Some(d2),
        Some(d3),
        NO_PIN,
    )
    .with_dma(dma_channel.configure(false, &mut descriptors, &mut [], DmaPriority::Priority0));

    let mut display = t_display_s3_amoled::rm67162::dma::RM67162Dma::new(spi, cs);
    display.reset(&mut rst, &mut delay).unwrap();
    display.init(&mut delay).unwrap();
    display
        .set_orientation(Orientation::Landscape)
        .unwrap();

    println!("display init ok");

    let window = MinimalSoftwareWindow::new(renderer::RepaintBufferType::ReusedBuffer);
    slint::platform::set_platform(Box::new(Backend {
        window: window.clone(),
    }))
    .unwrap();
    // window.dispatch_event(WindowEvent::ScaleFactorChanged { scale_factor: 2.0 });
    window.set_size(PhysicalSize::new(536, 240));

    let ui = AppWindow::new().unwrap();
    let _ui_handle = ui.as_weak();

    let mut frame_buffer = FramebufLiner { framebuf: [Rgb565PixelFlipped(0); 536*240] };

    let scale_ref = scale.clone();
    ui.global::<ScaleControls>().on_zero(move || {
        scale_ref.borrow_mut().rezero();
    });

    let recipes = [
        vegan_choux(),
        vegan_creme_pat(),
        choux(),
        creme_pat(),
    ];
    let progresses = recipes.iter().map(progress_for_recipe).collect::<Vec<_>>()[..].into();
    ui.set_recipes(recipes.into());
    ui.set_recipe_progresses(progresses);

    let mut i = 0;
    loop {
        loop {
            match event_consumer.dequeue() {
                Some(Event::WheelCW) => {
                    window.dispatch_event(WindowEvent::KeyPressed { text: Key::UpArrow.into() });
                    window.dispatch_event(WindowEvent::KeyReleased { text: Key::UpArrow.into() });
                },
                Some(Event::WheelCCW) => {
                    window.dispatch_event(WindowEvent::KeyPressed { text: Key::DownArrow.into() });
                    window.dispatch_event(WindowEvent::KeyReleased { text: Key::DownArrow.into() });
                },
                Some(Event::WheelButton(ButtonEvent::Press)) =>
                    window.dispatch_event(WindowEvent::KeyPressed { text: Key::RightArrow.into() }),
                Some(Event::WheelButton(ButtonEvent::Release)) =>
                    window.dispatch_event(WindowEvent::KeyReleased { text: Key::RightArrow.into() }),
                Some(Event::WheelButton(ButtonEvent::LongPress)) =>
                    window.dispatch_event(WindowEvent::KeyPressed { text: "d".into() }),
                Some(Event::WheelButton(ButtonEvent::LongRelease)) =>
                    window.dispatch_event(WindowEvent::KeyReleased { text: "d".into() }),
                Some(Event::BackButton(ButtonEvent::Press)) =>
                    window.dispatch_event(WindowEvent::KeyPressed { text: Key::LeftArrow.into() }),
                Some(Event::BackButton(ButtonEvent::Release)) =>
                    window.dispatch_event(WindowEvent::KeyReleased { text: Key::LeftArrow.into() }),
                Some(Event::BackButton(ButtonEvent::LongPress)) =>
                    window.dispatch_event(WindowEvent::KeyPressed { text: "b".into() }),
                Some(Event::BackButton(ButtonEvent::LongRelease)) =>
                    window.dispatch_event(WindowEvent::KeyReleased { text: "b".into() }),
                None =>
                    break
            }
        }

        if let Ok(val) = scale_adc.read() {
            let mut scale = scale.borrow_mut();
            scale.update(val);
            let cur_weight = scale.in_kg();
            println!("scale: {}", cur_weight);
        }
        // TODO: optimize
        ui.set_current_weight(scale.borrow().in_kg());

        slint::platform::update_timers_and_animations();

        i += 1;
        if i > 230 {
            i = 0;
        }

        // Draw the scene if something needs to be drawn.
        window.draw_if_needed(|renderer| {
            // renderer.render_by_line(&mut wrapper);
            let before_render = SystemTimer::now();
            renderer.render(&mut frame_buffer.framebuf[..], 536);
            // renderer.render_by_line(&mut frame_buffer);
            let after_render = SystemTimer::now();
            let _res = unsafe { display.fill_with_framebuffer(cast_pixel_buffer(&frame_buffer.framebuf[..])) };
            let after_fill = SystemTimer::now();
            println!("render: {}us", ((after_render - before_render) * 1_000_000) / SystemTimer::TICKS_PER_SECOND);
            println!("fill: {}us", ((after_fill - after_render) * 1_000_000) / SystemTimer::TICKS_PER_SECOND);
        });

        if !window.has_active_animations() {
            // if no animation is running, wait for the next input event
        }
    }
}
