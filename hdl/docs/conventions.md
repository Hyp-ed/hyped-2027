# HDL conventions

## Files and modules
- One module per file. The file is named after the module: `rtl/<name>.v`.
  The Makefile relies on this to find submodules.
- A module's testbench is `tb/tb_<name>.cpp`.
- Every file starts with `` `default_nettype none `` and ends with
  `` `default_nettype wire ``. This turns a misspelt signal name into an
  error instead of silently creating a new wire.
- Names are `snake_case`. Parameters are `UPPER_CASE`.

## Clock and reset
- One clock, `clk`, at **100 MHz** (provisional, until the FPGA is chosen).
- Reset is `rst`: **synchronous, active high**. Every register gets a defined reset value.
- Any signal from outside the FPGA passes through `synchroniser` before use.

## Module header
Every module starts with a comment block containing:
- what it does, in one or two sentences;
- each input and output: width, units and fixed-point format;
- latency in clock cycles, from input to output;
- anything the user of the module must know (limits, assumptions).

## Fixed-point formats
`Qm.n` here means a signed number with **m integer bits (including the sign
bit) and n fraction bits**, m + n bits in total.

| Quantity | Format | Bits | Range | Resolution | Status |
|---|---|---|---|---|---|
| Current | Q6.10 | 16 | ±32 A | ~1 mA | provisional |
| Voltage | Q11.5 | 16 | ±1024 V | ~31 mV | provisional (D2) |
| Gap | Q7.9 | 16 | ±64 mm | ~2 µm | provisional |
| PWM period and duty | unsigned integer | 16 | 0–65535 clock cycles | 1 cycle | agreed |
| Controller gains | Q16.16 | 32 | ±32768 | ~15 µ | placeholder, revisit once controller units are fixed |
Rules:
- Multiplying keeps full precision: the result width is the sum of the input widths.
- When narrowing a result, **round to nearest and saturate**. Never let a value
  silently wrap around.
- Document any other format used inside a module in its header.

## Testbenches
- C++, self-checking: print PASS or FAIL per check, and return non-zero if anything failed.
- Write `waves.vcd` (it lands in the module's build folder).
- Change inputs while the clock is low, so they're stable before the rising edge.
- Test normal, edge (zero, maximum, reset partway through) and fault cases.

## Before opening a PR
`make` passes: every module lints clean and every testbench passes.
