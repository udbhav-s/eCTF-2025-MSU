# MAX78000FTHR Template
This is a Cargo template for embedded Rust development with Analog Device's [MAX78000FTHR board](https://www.analog.com/en/resources/evaluation-hardware-and-software/evaluation-boards-kits/max78000fthr.html). It uses the [`max78000-hal`](https://github.com/sigpwny/max78000-hal) crate.

Currently, only the main ARM Cortex-M4 core is supported. The optional RISC-V core is not supported.

This template is based on the [`cortex-m-quickstart` template](https://github.com/rust-embedded/cortex-m-quickstart).

## Quick Resources
### Analog Devices docs
- [MAX78000FTHR Data Sheet](https://www.analog.com/media/en/technical-documentation/data-sheets/MAX78000FTHR.pdf)
- [MAX78000 Data Sheet](https://www.analog.com/media/en/technical-documentation/data-sheets/MAX78000.pdf)
- [MAX78000 User Guide](https://www.analog.com/media/en/technical-documentation/user-guides/max78000-user-guide.pdf)
### docs.rs
- [max7800x-hal](https://docs.rs/max7800x-hal) - Hardware Abstraction Layer for the MAX7800x family of microcontrollers
- [max78000-pac](https://docs.rs/max78000-pac) - Peripheral Access Crate for the MAX78000

## Dependencies
To build embedded programs using this template you'll need:

- Rust installed via `rustup`. See [installation instructions](https://www.rust-lang.org/tools/install).

- Rust 1.31, 1.30-beta, nightly-2018-09-13 or a newer toolchain.
```sh
rustup default beta
```

- The `cargo generate` subcommand. See [installation instructions](https://github.com/ashleygwilliams/cargo-generate#installation) or simply run:
```sh
cargo install cargo-generate
```

- `rust-std` components (pre-compiled `core` crate) for the ARM Cortex-M targets. For this MAX78000 template, we are using the `thumbv7em-none-eabihf` target, but you could also use `thumbv7em-none-eabi`.
```sh
rustup target add thumbv7em-none-eabihf
```

## Getting Started
> [!NOTE]  
> Be sure to check out the [embedded Rust book](https://rust-embedded.github.io/book) for additional guidance. It covers important things like flashing, running, and debugging programs in detail.

You can generate a new project using this template by running:
```
cargo generate --git https://github.com/sigpwny/max78000fthr-template.git
```

Alternatively, you can clone this repository and manually edit the `Cargo.toml` file to configure your project. You won't be able to build the project until you've replaced the placeholder values.

```toml
[package]
name = "{{project-name}}" # Be sure to replace this with your project name
version = "0.1.0"
authors = ["{{authors}}"] # Be sure to replace this with your name(s)
edition = "2021"
publish = false
```

## Building
You can build the contents in [`src`](./src) with:
```sh
cargo build
```

Alternatively, you can build some of the included example code in [`examples`](./examples) with:
```sh
# cargo build --example <example-name>
cargo build --example blinky
```

## Flashing
### Via DAPLink (OpenOCD + GDB)
> [!NOTE]  
> This flashing method is quick, but is intended only for rapid development. It will also only work for boards with debug enabled. For production, you should use other methods.

The MAX78000FTHR board includes a DAPLink debugger (via a MAX32625 chip) which sits between the Micro-USB port and the MAX78000 microcontroller. This DAPLink can be used to flash the MAX78000.

#### Prerequisites
- `openocd` - this has to be [Analog Device's custom fork of OpenOCD](https://github.com/analogdevicesinc/openocd/tree/release) since it includes flash and reset support for the MAX78000.
- `arm-none-eabi-gdb` - Arm GNU Toolchain

Both of the above tools can be installed via Analog Device's [MSDK](https://analogdevicesinc.github.io/msdk/USERGUIDE/#installation).

In one terminal, start OpenOCD with a GDB server using the following command. This will use the [`openocd.cfg`](./openocd.cfg) file to configure OpenOCD and automatically connect to the board.
```pwsh
# C:\MaximSDK\Tools\OpenOCD\openocd.exe
openocd.exe --search "C:/MaximSDK/Tools/OpenOCD/scripts"
```
In another terminal, start GDB with the following command:
```pwsh
# C:\MaximSDK\Tools\GNUTools\10.3\bin\arm-none-eabi-gdb.exe
arm-none-eabi-gdb.exe --command=openocd.gdb ./target/thumbv7em-none-eabihf/debug/{{project-name}}
```

The [`openocd.gdb`](./openocd.gdb) file contains the GDB commands to connect to OpenOCD's GDB server and flash the program (using the `load` command). Be sure to customize this file to your project's needs.

### Via DAPLink (Drag-and-Drop)
You will need to create a binary firmware file (`.bin`) from the built ELF file using `arm-none-eabi-objcopy` or `cargo-binutils`.

TODO: More instructions here.

### Via Custom Bootloader
If you are using a custom bootloader, you will need to format the generated ELF file into a format compatible with your bootloader (most likely, you'll need to use `arm-none-eabi-objcopy` or `cargo-binutils`). Refer to your bootloader's documentation for more information.

## Visual Studio Code
This template includes launch configurations for debugging Cortex-M programs with Visual Studio Code located in the `.vscode/` directory. See [.vscode/README.md](./.vscode/README.md) for more information.

If you're not using VS Code, you can safely delete the directory from the generated project.

## License
This template is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.