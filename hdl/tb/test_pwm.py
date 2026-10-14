"""cocotb testbench for rtl/pwm.v.

The Python equivalent of tb_pwm.cpp. Functions marked @cocotb.test() run
inside the simulation and drive the design; test_pwm() at the bottom is what
pytest runs, and it builds the design once per WIDTH value.

Run with:  make cocotb TOP=pwm
"""

import cocotb
import pytest
from cocotb.clock import Clock
from cocotb.triggers import ClockCycles, FallingEdge, RisingEdge

from cocotb_runner import run


async def start_and_reset(dut):
    """Start a 100 MHz clock and hold reset for a few cycles."""
    Clock(dut.clk, 10, unit="ns").start()
    dut.rst.value = 1
    dut.duty.value = 0
    await ClockCycles(dut.clk, 4)
    await FallingEdge(dut.clk)  # change inputs while the clock is low
    dut.rst.value = 0


@cocotb.test()
async def duty_cycle_matches_input(dut):
    """For several duty values, the output is high for exactly `duty` cycles per period."""
    await start_and_reset(dut)

    period = 2 ** len(dut.duty)  # 2^WIDTH, so this works for any WIDTH
    duties = [0, period // 4, period // 2, (period * 25) // 32, period - 1]

    for duty in duties:
        await FallingEdge(dut.clk)
        dut.duty.value = duty
        await ClockCycles(dut.clk, 2 * period)  # let the new duty take effect

        high_cycles = 0
        for _ in range(period):
            await RisingEdge(dut.clk)
            if dut.pwm_out.value == 1:
                high_cycles += 1

        dut._log.info(f"duty = {duty:3d} -> high for {high_cycles:3d} / {period} cycles")
        assert high_cycles == duty, f"duty {duty}: output high for {high_cycles} cycles"


@cocotb.test()
async def output_low_during_reset(dut):
    """While reset is held, the output stays low whatever the duty input."""
    Clock(dut.clk, 10, unit="ns").start()
    dut.rst.value = 1
    dut.duty.value = 2 ** len(dut.duty) - 1  # maximum duty
    await RisingEdge(dut.clk)  # a synchronous reset takes effect on the first edge
    for _ in range(20):
        await RisingEdge(dut.clk)
        assert dut.pwm_out.value == 0, "output went high during reset"


@pytest.mark.parametrize("width", [4, 8])
def test_pwm(width):
    run("pwm", "test_pwm", parameters={"WIDTH": width})
