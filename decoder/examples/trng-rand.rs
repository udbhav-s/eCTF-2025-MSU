#![no_std]
#![no_main]

pub extern crate max7800x_hal as hal;
pub use hal::pac;
pub use hal::entry;
use panic_halt as _;

use embedded_io::Write;
use rand::RngCore;

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

    // Create a new TRNG peripheral instance
    let mut trng = hal::trng::Trng::new(p.trng, &mut gcr.reg);

    let mut count = 0;
    loop {
        write!(console, "========================================\r\n").unwrap();
        write!(console, "Iteration {}\r\n", count).unwrap();
        // Generate a random 32-bit number
        let random_number = trng.next_u32();
        write!(console, "Random u32: {}\r\n", random_number).unwrap();
        // Fill an array with random bytes
        let mut buffer = [0u8; 16];
        trng.fill_bytes(&mut buffer);
        write!(console, "Random [u8; 16]: {:02x?}\r\n", buffer).unwrap();
        count += 1;
        delay.delay_ms(1000);
    }
}
