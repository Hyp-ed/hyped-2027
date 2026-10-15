"""cocotb testbench for rtl/pwm.v (centred PWM).

How it works:
  - PwmModel is a cycle-by-cycle Python model of what the header says pwm.v
    should do. A monitor runs in the background of every test and compares
    the real output with the model on every clock cycle.
  - Each test drives a scenario (duty changes, enable, reset) and adds its own
    readable checks on top, so a failure says what went wrong in plain terms.
  - test_pwm() at the bottom builds the design once per frequency setting.

Run with:  make cocotb TOP=pwm
"""

import os
import random

import cocotb
import pytest
from cocotb.clock import Clock
from cocotb.triggers import ClockCycles, FallingEdge, ReadOnly, RisingEdge

from cocotb_runner import run

CLK_FREQ_KHZ = int(os.environ.get("PARAM_CLK_FREQ_KHZ", "100000"))
PWM_FREQ_KHZ = int(os.environ.get("PARAM_PWM_FREQ_KHZ", "20"))
COUNT = CLK_FREQ_KHZ // PWM_FREQ_KHZ  # clock cycles per period


class PwmModel:
    """Reference model of pwm.v, following the spec in its header.

    clock() is called once per rising edge with the inputs as they were just
    before the edge, and returns what pwm_out should be just after it.
    """

    def __init__(self, count):
        self.count = count
        self.reset()

    def reset(self):
        self.counter = 0
        self.duty = 0          # latched duty, used for the whole period
        self.start = 0         # first counter value of the pulse
        self.enabled = False   # enable as sampled at the last period boundary
        self.out = 0

    def clock(self, rst, enable, duty):
        if rst:
            self.reset()
            return self.out

        in_pulse = self.start <= self.counter < self.start + self.duty
        self.out = int(enable and self.enabled and in_pulse)

        if self.counter == self.count - 1:  # end of period: take new settings
            self.duty = duty
            self.start = (self.count - duty) // 2 if duty < self.count else 0
            self.enabled = bool(enable)
            self.counter = 0
        else:
            self.counter += 1
        return self.out


async def monitor(dut, model, mismatches):
    """Compare pwm_out with the model on every clock cycle."""
    cycle = 0
    while True:
        await RisingEdge(dut.clk)
        # Inputs only change on falling edges, so these are the values the
        # design saw at this rising edge
        expected = model.clock(
            int(dut.rst.value), int(dut.enable.value), int(dut.duty.value)
        )
        await ReadOnly()  # let the edge's updates settle before reading
        actual = int(dut.pwm_out.value)
        if actual != expected:
            mismatches.append(f"cycle {cycle}: pwm_out = {actual}, expected {expected}")
        cycle += 1


async def setup(dut):
    """Start the clock and the model check, then reset. Returns the mismatch list."""
    Clock(dut.clk, 10, unit="ns").start()
    mismatches = []
    cocotb.start_soon(monitor(dut, PwmModel(COUNT), mismatches))

    dut.rst.value = 1
    dut.enable.value = 1
    dut.duty.value = 0
    await ClockCycles(dut.clk, 3)
    await FallingEdge(dut.clk)  # change inputs while the clock is low
    dut.rst.value = 0
    return mismatches


async def set_inputs(dut, **values):
    """Change inputs on a falling edge, so they're stable at the next rising edge."""
    await FallingEdge(dut.clk)
    for name, value in values.items():
        getattr(dut, name).value = value


def check_model(mismatches):
    shown = "\n".join(mismatches[:10])
    assert not mismatches, f"{len(mismatches)} cycles differ from the model:\n{shown}"


async def high_cycles_over_one_period(dut):
    """Count how many of the next COUNT cycles pwm_out is high."""
    high = 0
    for _ in range(COUNT):
        await RisingEdge(dut.clk)
        await ReadOnly()
        high += int(dut.pwm_out.value)
    return high


@cocotb.test()
async def output_low_during_reset(dut):
    """While reset is held, the output stays low whatever the inputs."""
    Clock(dut.clk, 10, unit="ns").start()
    dut.rst.value = 1
    dut.enable.value = 1
    dut.duty.value = COUNT
    await RisingEdge(dut.clk)  # a synchronous reset takes effect on the first edge
    for _ in range(2 * COUNT):
        await RisingEdge(dut.clk)
        await ReadOnly()
        assert int(dut.pwm_out.value) == 0, "output went high during reset"


@cocotb.test()
async def duty_sweep_gives_exact_high_time(dut):
    """Each duty gives exactly that many high cycles per period, clamped at COUNT."""
    mismatches = await setup(dut)
    max_duty = 2 ** len(dut.duty) - 1
    duties = sorted({0, 1, 2, COUNT // 4, COUNT // 2, COUNT - 1, COUNT,
                     min(COUNT + 3, max_duty), max_duty})

    for duty in duties:
        await set_inputs(dut, duty=duty)
        await ClockCycles(dut.clk, 2 * COUNT)  # let the new duty take effect
        high = await high_cycles_over_one_period(dut)
        expected = min(duty, COUNT)
        dut._log.info(f"duty {duty:5d} -> high for {high:5d} / {COUNT} cycles")
        assert high == expected, f"duty {duty}: high for {high} cycles, expected {expected}"

    check_model(mismatches)


@cocotb.test()
async def pulse_is_centred(dut):
    """Off-time before and after the pulse differs by at most one cycle."""
    mismatches = await setup(dut)

    for duty in sorted({1, COUNT // 3, COUNT // 2, COUNT // 2 + 1, COUNT - 2}):
        if duty <= 0:
            continue
        await set_inputs(dut, duty=duty)
        await ClockCycles(dut.clk, 2 * COUNT)

        # Record which counter values the pulse covers in one full period.
        # pwm_out lags the counter by one cycle, so the output seen after an
        # edge belongs to the counter value from before that edge.
        covered = set()
        for _ in range(COUNT):
            await RisingEdge(dut.clk)
            await ReadOnly()
            counter_before_edge = (int(dut.counter.value) - 1) % COUNT
            if int(dut.pwm_out.value):
                covered.add(counter_before_edge)

        off_before = min(covered)
        off_after = COUNT - 1 - max(covered)
        dut._log.info(f"duty {duty:5d}: off {off_before} before, {off_after} after")
        assert len(covered) == duty, f"duty {duty}: pulse is not one continuous block"
        assert off_after - off_before in (0, 1), (
            f"duty {duty}: off-time {off_before} before vs {off_after} after"
        )

    check_model(mismatches)


@cocotb.test()
async def duty_change_mid_period_is_glitch_free(dut):
    """Changing duty mid-period never produces a pulse of the wrong length."""
    mismatches = await setup(dut)
    await set_inputs(dut, duty=COUNT // 2)
    await ClockCycles(dut.clk, 2 * COUNT)

    for fraction in (0.1, 0.4, 0.5, 0.6, 0.9):
        await ClockCycles(dut.clk, int(COUNT * fraction))
        await set_inputs(dut, duty=random.randint(0, COUNT))
        await ClockCycles(dut.clk, 2 * COUNT)

    check_model(mismatches)  # the model latches duty only at period boundaries


@cocotb.test()
async def disable_turns_output_off_next_cycle(dut):
    """Disabling mid-pulse turns the output off on the very next clock edge."""
    mismatches = await setup(dut)
    await set_inputs(dut, duty=COUNT // 2)
    await ClockCycles(dut.clk, 2 * COUNT)

    while True:  # wait until a pulse is in progress
        await RisingEdge(dut.clk)
        await ReadOnly()
        if int(dut.pwm_out.value):
            break

    await set_inputs(dut, enable=0)
    await RisingEdge(dut.clk)
    await ReadOnly()
    assert int(dut.pwm_out.value) == 0, "output still high one cycle after disable"

    high = await high_cycles_over_one_period(dut)
    assert high == 0, f"output high for {high} cycles while disabled"
    check_model(mismatches)


@cocotb.test()
async def counter_keeps_running_while_disabled(dut):
    """While disabled, the period timing carries on, so the PWM stays in step."""
    mismatches = await setup(dut)
    await ClockCycles(dut.clk, COUNT // 3)

    await set_inputs(dut, enable=0)
    await ReadOnly()
    before = int(dut.counter.value)
    cycles = COUNT + COUNT // 2
    await ClockCycles(dut.clk, cycles)
    await ReadOnly()
    after = int(dut.counter.value)

    assert after == (before + cycles) % COUNT, (
        f"counter went {before} -> {after} in {cycles} cycles; "
        f"expected {(before + cycles) % COUNT}"
    )
    check_model(mismatches)


@cocotb.test()
async def reenable_waits_for_next_period(dut):
    """Re-enabling mid-period keeps the output off until the next period starts."""
    mismatches = await setup(dut)
    await set_inputs(dut, duty=COUNT)  # 100%: any partial pulse would show at once
    await ClockCycles(dut.clk, 2 * COUNT)
    await set_inputs(dut, enable=0)
    await ClockCycles(dut.clk, COUNT + COUNT // 3)

    await set_inputs(dut, enable=1)  # partway through a period
    await ReadOnly()
    # Includes the edge at the boundary itself: pwm_out lags the counter by a
    # cycle, so the output after that edge still belongs to the old period
    cycles_until_new_period = COUNT - int(dut.counter.value)
    for _ in range(cycles_until_new_period):
        await RisingEdge(dut.clk)
        await ReadOnly()
        assert int(dut.pwm_out.value) == 0, "output came back before the period boundary"

    high = await high_cycles_over_one_period(dut)
    assert high == COUNT, f"first full period after re-enable: high for {high} of {COUNT}"
    check_model(mismatches)


@cocotb.test()
async def random_stress(dut):
    """Random duty and enable changes at random times, checked against the model."""
    mismatches = await setup(dut)
    max_duty = 2 ** len(dut.duty) - 1
    remaining = max(20 * COUNT, 20_000)

    while remaining > 0:
        wait = random.randint(1, 2 * COUNT)
        await ClockCycles(dut.clk, wait)
        remaining -= wait
        change = random.random()
        if change < 0.7:
            await set_inputs(dut, duty=random.choice(
                [0, COUNT, max_duty, random.randint(0, COUNT)]))
        elif change < 0.9:
            await set_inputs(dut, enable=random.randint(0, 1))
        else:  # reset in the middle of running
            await set_inputs(dut, rst=1)
            await ClockCycles(dut.clk, 2)
            await set_inputs(dut, rst=0)

    check_model(mismatches)


@pytest.mark.parametrize(
    "clk_khz, pwm_khz",
    [
        (1_000, 100),    # COUNT = 10: small, fast, even
        (1_700, 100),    # COUNT = 17: odd period, so off-times split unevenly
        (1_600, 100),    # COUNT = 16: a power of two, checks WIDTH can hold COUNT
        (100_000, 20),   # COUNT = 5000: the real 20 kHz at 100 MHz
    ],
)
def test_pwm(clk_khz, pwm_khz):
    run("pwm", "test_pwm",
        parameters={"CLK_FREQ_KHZ": clk_khz, "PWM_FREQ_KHZ": pwm_khz})
