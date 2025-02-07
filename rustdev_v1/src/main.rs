#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
pub use hal::pac;
pub use hal::entry;

// pick a panicking behavior
use panic_halt as _; // you can put a breakpoint on `rust_begin_unwind` to catch panics
// use panic_abort as _; // requires nightly
// use panic_itm as _; // logs messages over ITM; requires ITM support
// use panic_semihosting as _; // logs messages to the host stderr; requires a debugger
// use cortex_m_semihosting::heprintln; // uncomment to use this for printing through semihosting

#[entry]
fn main() -> ! {
    // heprintln!("Hello from semihosting!");
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk
        .set_source(&mut gcr.reg, &ipo)
        .set_divider::<hal::gcr::clocks::Div1>(&mut gcr.reg)
        .freeze();

    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    let rate = clks.sys_clk.frequency;
    let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let console = hal::uart::UartPeripheral::uart0(
        p.uart0,
        &mut gcr.reg,
        rx_pin,
        tx_pin
    )
        .baud(115200)
        .clock_pclk(&clks.pclk)
        .parity(hal::uart::ParityBit::None)
        .build();

    console.write_bytes(b"Hello, world!\r\n");

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
