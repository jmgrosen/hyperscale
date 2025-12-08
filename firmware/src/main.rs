// Copyright (C) Jessie Grosen 2024
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]

extern crate alloc;

use core::cell::RefCell;
use core::iter;
use core::mem;
use core::slice;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use critical_section::Mutex;
use embedded_hal_1::delay::DelayNs;
use embedded_hal_1::digital::InputPin;
use embedded_hal_1::i2c::I2c as I2cTrait;
use esp_backtrace as _;
use esp_println::println;
use esp_hal::{
    clock::CpuClock,
    timer::PeriodicTimer,
    timer::timg::TimerGroup, delay::Delay, rtc_cntl::Rtc,
    gpio::{Io, Input, InputConfig, Output, OutputConfig, Level, Pull, self},
    i2c::master::{I2c, Config as I2cConfig},
    spi::{Mode as SpiMode, master::{Spi, Config as SpiConfig}},
    time::{self, Rate},
    handler,
};

use nau7802::AfeCalibrationStatus;
use rotary_encoder_hal::{Direction, Rotary, DefaultPhase};
use nau7802::Nau7802;
use heapless::spsc::{Queue, Producer};
use debouncr::{DebouncerStateful, Edge, Repeat6, debounce_stateful_6};

use slint::platform::software_renderer::RenderingRotation;
use slint::platform::software_renderer::{MinimalSoftwareWindow, Rgb565Pixel, TargetPixel, PremultipliedRgbaColor};
use slint::platform::{software_renderer as renderer, Platform, WindowEvent, Key};
use slint::{Model, PhysicalSize};

use t_display_s3_amoled::rm67162::dma::RM67162Dma;
use t_display_s3_amoled::rm67162::Orientation;

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
enum Event {
    WheelCW,
    WheelCCW,
    WheelButton(ButtonEvent),
    BackButton(ButtonEvent),
}

struct InterruptResources {
    encoder: Rotary<Input<'static>, Input<'static>, DefaultPhase>,
    producer: Producer<'static, Event, 16>,
    wheel_button: Button<Input<'static>>,
    back_button: Button<Input<'static>>,
    timer: PeriodicTimer<'static, esp_hal::Blocking>,
}

static INTERRUPT_RESOURCES: Mutex<RefCell<Option<InterruptResources>>> = Mutex::new(RefCell::new(None));

#[handler]
fn gpio_handler() {
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

const TIMER_PERIOD_MS: u64 = 5;

#[handler]
fn timer0_handler() {
    critical_section::with(|cs| {
        let mut borrowed_resources = INTERRUPT_RESOURCES.borrow_ref_mut(cs);
        let resources = borrowed_resources.as_mut().unwrap();

        resources.timer.clear_interrupt();

        if let Some(event) = resources.wheel_button.update() {
            let _ = resources.producer.enqueue(Event::WheelButton(event));
        }
        if let Some(event) = resources.back_button.update() {
            let _ = resources.producer.enqueue(Event::BackButton(event));
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
        core::time::Duration::from_micros(
            time::Instant::now().duration_since_epoch().as_micros()
        )
    }

    // fn run_event_loop(&self) -> Result<(), slint::PlatformError>
    fn debug_log(&self, arguments: core::fmt::Arguments) {
        println!("Slint: {:?}", arguments);
    }
}

/*
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
*/

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

#[derive(Default)]
enum Scale<I: I2cTrait> {
    #[default]
    Empty,
    Unconnected(I),
    Calibrating(Nau7802<I>),
    Running { adc: Nau7802<I>, zero: i32, val: i32 },
}

impl<I: I2cTrait> Scale<I> {
    fn step_inner(self, wait: &mut impl DelayNs) -> (Self, Option<f32>) {
        use Scale::*;
        match self {
            Empty =>
                // this shouldn't happen :)
                (Empty, None),
            Unconnected(i2c) =>
                match Nau7802::new(i2c, wait) {
                    Ok(adc) =>
                        (Calibrating(adc), None),
                    Err((_, i2c)) =>
                        (Unconnected(i2c), None),
                },
            Calibrating(mut adc) =>
                if let Ok(AfeCalibrationStatus::Success) = adc.poll_afe_calibration_status() {
                    if let Ok(val) = adc.read() {
                        (Running { adc, zero: val, val }, Some(0.))
                    } else {
                        (Calibrating(adc), None)
                    }
                } else {
                    (Calibrating(adc), None)
                },
            Running { mut adc, zero, val } => {
                let new_val = adc.read().unwrap_or(val);
                (Running { adc, zero, val: new_val }, Some(((new_val - zero) as f32) * ONE_KG))
            },
        }
    }

    /// Steps the connecting/calibrating/running state
    /// machine. Returns the most recent reading if we have calibrated
    /// successfully.
    fn step(&mut self, wait: &mut impl DelayNs) -> Option<f32> {
        let real_self = mem::take(self);
        let (new_self, result) = real_self.step_inner(wait);
        let _ = mem::replace(self, new_self);
        result
    }

    fn rezero(&mut self) {
        if let &mut Scale::Running { ref mut zero, val, .. } = self {
            *zero = val;
        }
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
            // ingredient("soy milk", 0.243),
            // ingredient("vanilla extract", 0.010),
            // ingredient("salt", 0.001),
            // ingredient("corn starch", 0.016),
            // ingredient("sugar", 0.050),
            // ingredient("Just Egg", 0.083),
            // ingredient("vegan butter", 0.042),
            ingredient("soy milk", 0.486),
            ingredient("vanilla extract", 0.020),
            ingredient("salt", 0.002),
            ingredient("corn starch", 0.032),
            ingredient("sugar", 0.100),
            ingredient("Just Egg", 0.166),
            ingredient("vegan butter", 0.084)
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
            ingredient("milk", 0.455),
            ingredient("vanilla bean", 0.001),
            ingredient("sugar", 0.115),
            ingredient("corn starch", 0.030),
            ingredient("salt", 0.001),
            ingredient("egg yolks", 0.070),
            ingredient("butter", 0.030),
        ].into(),
    }
}

fn pasta_dough() -> Recipe {
    Recipe {
        name: "Egg Pasta".into(),
        ingredients: [
            ingredient("flour", 0.255),
            ingredient("whole eggs", 0.110),
            ingredient("egg yolks", 0.070),
            ingredient("salt", 0.003),
        ].into(),
    }
}

fn poolish_bread() -> Recipe {
    Recipe {
        name: "Poolish Bread".into(),
        ingredients: [
            ingredient("flour", 0.5),
            ingredient("yeast", 0.0004),
            ingredient("water (80F)", 0.5),
            ingredient("flour", 0.5),
            ingredient("salt", 0.021),
            ingredient("yeast", 0.003),
            ingredient("water (105F)", 0.25),
        ].into(),
    }
}

fn focaccia() -> Recipe {
    Recipe {
        name: "Focaccia".into(),
        ingredients: [
            ingredient("flour", 0.5),
            ingredient("salt", 0.01),
            ingredient("yeast", 0.004),
            ingredient("water (roomtemp)", 0.4),
            ingredient("olive oil", 0.02),
            ingredient("olive oil", 0.028),
            ingredient("olive oil", 0.02),
        ].into(),
    }
}

fn kouign_amann() -> Recipe {
    Recipe {
        name: "Kouign Amann".into(),
        ingredients: [
            ingredient("flour", 0.213),
            ingredient("salt", 0.0032),
            ingredient("yeast", 0.0016),
            ingredient("water (75F)", 0.145),
            ingredient("salted butter", 0.134),
            ingredient("sugar", 0.156),
        ].into(),
    }
}

fn pie_dough() -> Recipe {
    Recipe {
        name: "Pie Dough".into(),
        ingredients: [
            ingredient("low-protein APF", 0.225),
            ingredient("sugar", 0.015),
            ingredient("salt", 0.004),
            ingredient("unsalted butter", 0.225),
            ingredient("cold tap water", 0.115),
        ].into(),
    }
}

fn butternut_pie() -> Recipe {
    Recipe {
        name: "Butternut Pie".into(),
        ingredients: [
            ingredient("butternut puree", 0.395),
            ingredient("condensed milk", 0.680),
            ingredient("light brown sugar", 0.115),
            ingredient("vanilla extract", 0.015),
            ingredient("3/2tsp ground ginger", 0.001),
            ingredient("3/2tsp ground cinnamon", 0.001),
            ingredient("1/4tsp grated nutmeg", 0.001),
            ingredient("salt", 0.001),
            ingredient("1/8tsp ground cloves", 0.001),
            ingredient("unsalted butter", 0.030),
            ingredient("eggs", 0.145),
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

#[esp_hal::main]
fn main() -> ! {
    // init_heap();
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);
    println!("initted psram");

    // Disable the RTC and TIMG watchdog timers
    let mut rtc = Rtc::new(peripherals.LPWR);
    let timer_group0 = TimerGroup::new(peripherals.TIMG0);
    let mut wdt0 = timer_group0.wdt;
    let timer_group1 = TimerGroup::new(peripherals.TIMG1);
    let mut wdt1 = timer_group1.wdt;
    rtc.rwdt.disable();
    wdt0.disable();
    wdt1.disable();
    println!("Hello board!");

    // Set GPIO4 as an output, and set its state high initially.
    let mut io = Io::new(peripherals.IO_MUX);
    io.set_interrupt_handler(gpio_handler);
    // let mut led = io.pins.gpio38.into_push_pull_output();
    // let _button = io.pins.gpio21.into_pull_down_input();

    // led.set_high().unwrap();

    // Initialize the Delay peripheral, and use it to toggle the LED state in a
    // loop.
    let mut delay = Delay::new();

    // set up rotary encoder
    let pullup = InputConfig::default().with_pull(Pull::Up);
    let mut pin_a = Input::new(peripherals.GPIO1, pullup);
    let mut pin_b = Input::new(peripherals.GPIO2, pullup);
    let event_queue: &'static mut Queue<Event, 16> = {
        static mut Q: Queue<Event, 16> = Queue::new();
        unsafe { &mut Q }
    };
    let (event_producer, mut event_consumer) = event_queue.split();

    let encoder_button = Input::new(peripherals.GPIO3, pullup);
    let back_button = Input::new(peripherals.GPIO10, pullup);

    let mut tearing_effect = Input::new(peripherals.GPIO9, InputConfig::default());
    
    let timer00 = timer_group0.timer0;
    critical_section::with(|cs| {
        pin_a.listen(gpio::Event::AnyEdge);
        pin_b.listen(gpio::Event::AnyEdge);
        let rotary = Rotary::new(pin_a, pin_b);
        let mut periodic_timer = PeriodicTimer::new(timer00);
        periodic_timer.set_interrupt_handler(timer0_handler);
        periodic_timer.listen();
        periodic_timer.start(time::Duration::from_millis(TIMER_PERIOD_MS)).unwrap();
        INTERRUPT_RESOURCES.borrow_ref_mut(cs).replace(InterruptResources {
            encoder: rotary,
            producer: event_producer,
            wheel_button: Button::new(encoder_button),
            back_button: Button::new(back_button),
            timer: periodic_timer,
        });
    });

    let i2c_config = I2cConfig::default().with_frequency(Rate::from_khz(100));
    let i2c = I2c::new(peripherals.I2C0, i2c_config).unwrap()
        .with_sda(peripherals.GPIO43)
        .with_scl(peripherals.GPIO44);
    let scale = Rc::new(RefCell::new(Scale::Unconnected(i2c)));

    println!("init display");

    let sclk = peripherals.GPIO47;
    let rst = peripherals.GPIO17;
    let cs = peripherals.GPIO6;

    let d0 = peripherals.GPIO18;
    let d1 = peripherals.GPIO7;
    let d2 = peripherals.GPIO48;
    let d3 = peripherals.GPIO5;

    let cs = Output::new(cs, Level::High, OutputConfig::default());

    let mut rst = Output::new(rst, Level::Low, OutputConfig::default());

    let spi = Spi::new(
        peripherals.SPI2, // use spi2 host
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(75)) // max 75MHz
            .with_mode(SpiMode::_0),
        )
        .unwrap()
        .with_sck(sclk)
        .with_sio0(d0)
        .with_sio1(d1)
        .with_sio2(d2)
        .with_sio3(d3)
        .with_dma(peripherals.DMA_CH0);

    let mut display = RM67162Dma::new(spi, cs);
    display.reset(&mut rst, &mut delay).unwrap();
    display.init(&mut delay).unwrap();
    display
        .set_orientation(Orientation::Portrait)
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

    let mut framebuf = [Rgb565PixelFlipped(0); 536*240];

    let scale_ref = scale.clone();
    ui.global::<ScaleControls>().on_zero(move || {
        scale_ref.borrow_mut().rezero();
    });

    let recipes = [
        vegan_choux(),
        vegan_creme_pat(),
        choux(),
        creme_pat(),
        pasta_dough(),
        poolish_bread(),
        focaccia(),
        kouign_amann(),
        pie_dough(),
        butternut_pie(),
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

        let cur_weight = if let Some(weight) = scale.borrow_mut().step(&mut delay) {
            ScaleStatus { valid: true, weight }
        } else {
            ScaleStatus { valid: false, weight: 0. }
        };
        ui.set_current_weight(cur_weight);

        slint::platform::update_timers_and_animations();

        i += 1;
        if i > 230 {
            i = 0;
        }

        // Draw the scene if something needs to be drawn.
        window.draw_if_needed(|renderer| {
            renderer.set_rendering_rotation(RenderingRotation::Rotate90);
            // renderer.render_by_line(&mut wrapper);
            let before_render = now_us();
            renderer.render(&mut framebuf[..], 240);
            // renderer.render_by_line(&mut frame_buffer);
            let after_render = now_us();
            while tearing_effect.is_high() { }
            while tearing_effect.is_low() { }
            let after_wait = now_us();
            let _res = unsafe { display.fill_with_framebuffer(cast_pixel_buffer(&framebuf[..])) };
            let after_fill = now_us();
            println!(
                "render: {}us, wait: {}us, fill: {}us",
                after_render.wrapping_sub(before_render),
                after_wait.wrapping_sub(after_render),
                after_fill.wrapping_sub(after_wait),
            );
        });

        if !window.has_active_animations() {
            // if no animation is running, wait for the next input event
        }
    }
}

fn now_us() -> u64 {
    time::Instant::now().duration_since_epoch().as_micros()
}
