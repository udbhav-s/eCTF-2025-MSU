#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
pub use hal::pac;
pub use hal::entry;
use panic_halt as _;

// embedded_io API allows usage of core macros like `write!`
use embedded_io::Write;

#[entry]
fn main() -> ! {
    // Take ownership of the MAX78000 peripherals
    let p = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();
    // Initialize system peripherals and clocks
    let mut gcr = hal::gcr::Gcr::new(p.gcr, p.lpgcr);
    let ipo = hal::gcr::clocks::Ipo::new(gcr.osc_guards.ipo).enable(&mut gcr.reg);
    let clks = gcr.sys_clk.set_source(&mut gcr.reg, &ipo).freeze();
    // Initialize a delay timer using the ARM SYST (SysTick) peripheral
    let rate = clks.sys_clk.frequency;
    let mut delay = cortex_m::delay::Delay::new(core.SYST, rate);

    // Initialize and split the GPIO0 peripheral into pins
    let gpio0_pins = hal::gpio::Gpio0::new(p.gpio0, &mut gcr.reg).split();
    // Configure UART to host computer with 115200 8N1 settings
    let rx_pin = gpio0_pins.p0_0.into_af1();
    let tx_pin = gpio0_pins.p0_1.into_af1();
    let mut console = hal::uart::UartPeripheral::uart0(
        p.uart0,
        &mut gcr.reg,
        rx_pin,
        tx_pin
    )
        .baud(115200)
        .clock_pclk(&clks.pclk)
        .parity(hal::uart::ParityBit::None)
        .build();

    // You can use the HAL API directly (format strings not supported)...
    console.write_bytes(b"Hello, world!\r\n");
    // ... or you can use the embedded_io API (format strings supported)!
    let answer = 42;
    write!(console, "The answer is {}\r\n", answer).unwrap();

    // Test countdown with UART and delay
    let mut countdown = 10;
    while countdown >= 0 {
        write!(console, "{}\r\n", countdown).unwrap();
        delay.delay_ms(1000);
        countdown -= 1;
    }

    // UART echo loop
    loop {
        let byte = console.read_byte();
        console.write_byte(byte);
    }
}
