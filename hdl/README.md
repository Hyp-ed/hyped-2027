# HYPED HDL

Verilog designs for the HYPED FPGA team, simulated with
[Verilator](https://verilator.org) and viewed in [Surfer](https://surfer-project.org).

## Folder layout

```
hdl/
├── Makefile        build, run, lint, view waveforms
├── rtl/pwm.v       the hardware design (Verilog)
└── tb/tb_pwm.cpp   the testbench (C++): drives the design and checks its output
```

`rtl/pwm.v` is a PWM generator: a square wave whose duty cycle (the fraction of
time it's high) is set by an input. It's the first building block of the
levitation current-control project.

## How it works

1. **Verilator converts the Verilog into a C++ class** (`Vpwm`). Every input and
   output of the module becomes a variable on that class.
2. **The testbench is an ordinary C++ program.** It creates a `Vpwm`, sets its
   inputs, toggles the clock by hand (`tick()`), and reads the outputs.
3. **The testbench checks itself.** For each duty value it counts how many cycles
   the output is high and compares that with what it should be. It prints PASS or
   FAIL and exits with an error if anything fails, so it can run in CI.
4. **Every signal is recorded** into `waves.vcd`, which you open in Surfer to see
   the waveforms.

## Setup

### Install Verilator (version 5 or newer)

| OS | Command |
|---|---|
| macOS | `brew install verilator` |
| Ubuntu 24.04+ | `sudo apt install verilator` |
| Windows | Install WSL (Ubuntu), then follow the Ubuntu line inside WSL |

Check it with `verilator --version`. The testbench needs **5.x**. Older Ubuntu
releases ship 4.x, which is too old; build from source if your version starts
with 4.

You also need `make` and a C++ compiler. On macOS, run `xcode-select --install`;
on Ubuntu, `sudo apt install build-essential`.

### Install Surfer (waveform viewer)

Use either or both:

- **VS Code extension:** search "Surfer" in the Extensions panel. After that,
  opening a `.vcd` file in VS Code shows the waveform.
- **Desktop app:** `brew install surfer` on macOS, or download it from
  [surfer-project.org](https://surfer-project.org).

### Recommended VS Code extensions

- **Verilog-HDL/SystemVerilog/Bluespec SystemVerilog** (mshr-h): syntax
  highlighting, and it can run Verilator lint as you type.
- **C/C++** (Microsoft): for the testbench.

The testbench includes `Vpwm.h`, which only exists after the first build, so VS
Code shows red squiggles on the `#include` lines until you've run `make` once.
That's expected.

## Running

From inside the `hdl/` folder:

| Command | What it does |
|---|---|
| `make` | Build and run the testbench |
| `make lint` | Check the Verilog for mistakes without simulating |
| `make waves` | Open the waveform in the Surfer desktop app |
| `make clean` | Delete everything generated |

Expected output of `make`:

```
duty =   0  ->  high for   0 / 256 cycles (  0.0%)  PASS
duty =  64  ->  high for  64 / 256 cycles ( 25.0%)  PASS
duty = 128  ->  high for 128 / 256 cycles ( 50.0%)  PASS
duty = 200  ->  high for 200 / 256 cycles ( 78.1%)  PASS
duty = 255  ->  high for 255 / 256 cycles ( 99.6%)  PASS

All tests passed. Waveform written to waves.vcd in the build folder
```

`make` only rebuilds when `pwm.v` or `tb_pwm.cpp` has changed.

### Where the build goes

Normally everything is generated in `hdl/build/` (git ignores it), and the
waveform is `hdl/build/waves.vcd`.

**If the folder path contains a space**, Verilator refuses to build there. The
Makefile detects this and builds in `~/.cache/hyped-hdl/` instead, so the
waveform is `~/.cache/hyped-hdl/waves.vcd`. `make waves` finds it either way.
To open it with the VS Code extension, use *File → Open* on that path; on macOS,
press `Cmd+Shift+.` in the file picker to show hidden folders such as `.cache`.

## Viewing the waveform

1. Run `make`, then `make waves` (or open `waves.vcd` in VS Code).
2. In Surfer, expand `TOP → pwm` in the left panel and add `clk`, `duty`,
   `counter`, `duty_latched` and `pwm_out`.
3. Zoom out to see several periods. You can see:
   - `counter` ramping 0 → 255 and wrapping around;
   - `pwm_out` staying high while `counter < duty_latched`;
   - the pulse getting wider each time `duty` steps up;
   - `duty_latched` only changing at the end of a period, even though `duty`
     changes in the middle of one. That's the glitch protection.

## Demo script

1. Open `rtl/pwm.v` and walk through it: inputs and outputs, the counter, the
   comparison, and why duty is latched once per period.
2. Open `tb/tb_pwm.cpp`: creating the model, `tick()`, the reset, the test loop
   and the PASS/FAIL check.
3. Run `make` and show all tests passing.
4. Run `make waves` and look at the signals.
5. **Break it on purpose:** in `pwm.v`, change `(counter < duty_latched)` to
   `(counter <= duty_latched)` and run `make` again. Every test fails with one
   cycle too many, which is an off-by-one error. Change it back and they pass.
   This is the point of a self-checking testbench: the bug is caught
   automatically, without anyone staring at waveforms.

## Adding a new module

1. Put the design in `rtl/<name>.v` and the testbench in `tb/tb_<name>.cpp`.
2. In the `Makefile`, set `TOP`, `RTL` and `TB` to the new files. The generated
   class is called `V<name>`, so include `V<name>.h` in the testbench.
3. Run `make lint` first, then `make`.

Once there are several modules, the Makefile will grow a target per module.
