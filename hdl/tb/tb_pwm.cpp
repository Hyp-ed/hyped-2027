// C++ testbench for rtl/pwm.v
//
// Verilator converts pwm.v into a C++ class called `Vpwm`. This file creates
// that model, drives its inputs, toggles the clock, and checks the output.
// It also records every signal into waves.vcd (in the build folder) so you
// can view the waveforms in Surfer.

#include <cstdio>
#include <memory>

#include "Vpwm.h"
#include "verilated.h"
#include "verilated_vcd_c.h"

static constexpr int PERIOD = 256;  // 2^WIDTH with the default WIDTH = 8

int main(int argc, char** argv) {
    auto ctx = std::make_unique<VerilatedContext>();
    ctx->commandArgs(argc, argv);
    ctx->traceEverOn(true);

    auto dut = std::make_unique<Vpwm>(ctx.get());

    auto trace = std::make_unique<VerilatedVcdC>();
    dut->trace(trace.get(), 99);
    trace->open("waves.vcd");  // written into the build folder

    // One full clock cycle: falling edge then rising edge, recording both.
    auto tick = [&]() {
        dut->clk = 0;
        dut->eval();
        ctx->timeInc(1);
        trace->dump(ctx->time());

        dut->clk = 1;
        dut->eval();
        ctx->timeInc(1);
        trace->dump(ctx->time());
    };

    // Reset for a few cycles
    dut->rst = 1;
    dut->duty = 0;
    for (int i = 0; i < 4; i++) tick();
    dut->rst = 0;

    // For each duty value: apply it, wait two periods for it to take effect,
    // then count how many cycles the output is high over exactly one period.
    const int duties[] = {0, 64, 128, 200, 255};
    int failures = 0;

    for (int duty : duties) {
        dut->duty = duty;
        for (int i = 0; i < 2 * PERIOD; i++) tick();

        int high_cycles = 0;
        for (int i = 0; i < PERIOD; i++) {
            tick();
            high_cycles += dut->pwm_out;
        }

        bool pass = (high_cycles == duty);
        if (!pass) failures++;
        std::printf("duty = %3d  ->  high for %3d / %d cycles (%5.1f%%)  %s\n",
                    duty, high_cycles, PERIOD, 100.0 * high_cycles / PERIOD,
                    pass ? "PASS" : "FAIL");
    }

    trace->close();
    dut->final();

    if (failures == 0) {
        std::printf("\nAll tests passed. Waveform written to waves.vcd in the build folder\n");
        return 0;
    }
    std::printf("\n%d test(s) FAILED\n", failures);
    return 1;
}
