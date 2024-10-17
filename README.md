# Hyperscale

A kitchen scale that helps you follow a recipe.

## Building

Export Gerbers from KiCAD for the PCB. Build STLs from OpenSCAD for the case.

An actual assembly guide is TODO!

For the firmware, install [the Xtensa rustc target](https://docs.esp-rs.org/book/installation/riscv-and-xtensa.html) and [espflash](https://docs.esp-rs.org/book/tooling/espflash.html) as described in the Rust on ESP Book. Then `cargo espflash flash`.

## Licensing

The physical design files are licensed under the CERN Open Hardware Licence Version 2 - Permissive.

The firmware code is licensed under MIT. However, due to its use of Slint, the resulting firmware as a whole is licensed under GPLv3.
