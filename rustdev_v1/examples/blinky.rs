#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
pub use hal::pac;
pub use hal::entry;
use panic_halt as _;

#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    // Constrain the Global Control Register (GCR) peripheral
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    // Initialize the Internal Primary Oscillator (IPO)
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    // Set the system clock to the IPO
    let clks = gcr.sys_clk.set_source(&mut gcr.reg, &ipo).freeze();
    // Initialize a delay timer using the ARM SysTick peripheral
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clks.sys_clk.frequency);

    // Initialize the GPIO2 peripheral
    let pins = hal::gpio::Gpio2::new(p.gpio2, &mut gcr.reg).split();
    // Enable output mode for the RGB LED pins
    let mut led_r = pins.p2_0.into_input_output();
    let mut led_g = pins.p2_1.into_input_output();
    let mut led_b = pins.p2_2.into_input_output();
    // Use VDDIOH as the power source for the RGB LED pins (3.0V)
    // Note: This HAL API may change in the future
    led_r.set_power_vddioh();
    led_g.set_power_vddioh();
    led_b.set_power_vddioh();

    // LED blink loop
    loop {
        led_r.set_high();
        delay.delay_ms(500);
        led_g.set_high();
        delay.delay_ms(500);
        led_b.set_high();
        delay.delay_ms(500);
        led_r.set_low();
        delay.delay_ms(500);
        led_g.set_low();
        delay.delay_ms(500);
        led_b.set_low();
        delay.delay_ms(500);
    }
}
