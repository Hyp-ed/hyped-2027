"""cocotb testbench for rtl/pi_controller.sv

How it works (see docs/pi_controller_vplan.md for the full plan):
  - A test builds PiSample transactions and drive_sample() puts each one on
    the pins with a one-cycle sample_valid pulse.
  - input_monitor() watches the pins. Each time sample_valid is high it runs
    the sample through PiModel (tb/models/pi_model.py) and queues
    the expected (out, saturated).
  - output_monitor() watches out_valid. Each output is compared with the
    oldest expected result.
  - check_scoreboard() at the end of a test fails on wrong outputs, outputs
    that weren't expected, and expected outputs that never arrived.
  - Tests add their own readable checks on top, so a failure says what went
    wrong in plain terms.

The model works one sample at a time, so it can't predict `clear` arriving
in the middle of an update. Random tests only change `clear` between updates;
clear_timing checks the in-flight case with values worked out by hand.

All values are raw fixed-point integers (see fixed_point_pkg.sv):
  current Q6.10 (1 A = 1024), gain Q16.16 (1.0 = 65536), output Q16.0 cycles.

Run with:  make cocotb TOP=pi_controller
"""

import random
from collections import deque
from dataclasses import dataclass

import cocotb
from cocotb.clock import Clock
from cocotb.triggers import ClockCycles, FallingEdge, ReadOnly, RisingEdge

from cocotb_runner import run
from models.pi_model import PiModel

AMP = 1 << 10     # 1 A in Q6.10
GAIN = 1 << 16    # gain 1.0 in Q16.16
CYCLE = 1 << 16   # 1 output cycle in the integrator's Q.16

CURRENT_MAX, CURRENT_MIN = 2**15 - 1, -(2**15)
GAIN_MAX, GAIN_MIN = 2**31 - 1, -(2**31)
OUTPUT_MAX, OUTPUT_MIN = 2**15 - 1, -(2**15)

TIMEOUT_US = 500  # a hung test fails instead of running forever
LATENCY = 4  # cycles from sample_valid to out_valid (rule A1 in the plan)


@dataclass
class PiSample:
    """One controller update: everything the DUT captures on sample_valid."""
    setpoint: int
    measured: int
    kp: int = 0
    ki: int = 0
    out_min: int = 0
    out_max: int = 1000


@dataclass
class Scoreboard:
    expected: deque
    errors: list
    outputs: int = 0
    checking: bool = True


# --- driver ---

async def drive_sample(dut, sample):
    """Put a sample on the pins and pulse sample_valid for one cycle.

    Returns on the falling edge after the pulse, without waiting for the result.
    """
    await FallingEdge(dut.clk)
    dut.setpoint.value = sample.setpoint
    dut.measured.value = sample.measured
    dut.kp.value = sample.kp
    dut.ki.value = sample.ki
    dut.out_min.value = sample.out_min
    dut.out_max.value = sample.out_max
    dut.sample_valid.value = 1
    await FallingEdge(dut.clk)
    dut.sample_valid.value = 0


async def wait_for_output(dut):
    """Wait for out_valid and return (out, saturated)."""
    while True:
        await RisingEdge(dut.clk)
        await ReadOnly()
        if dut.out_valid.value == 1:
            return dut.out.value.to_signed(), int(dut.saturated.value)


async def run_sample(dut, sample):
    """Drive one sample and wait for its result. The next sample can follow straight away."""
    await drive_sample(dut, sample)
    return await wait_for_output(dut)


async def set_inputs(dut, **values):
    """Change inputs on a falling edge, so they're stable at the next rising edge."""
    await FallingEdge(dut.clk)
    for name, value in values.items():
        getattr(dut, name).value = value


# --- monitors and scoreboard ---

async def input_monitor(dut, model, sb):
    """Run every sample the DUT sees through the model and queue the result."""
    in_flight = 0  # cycles until the current update finishes (0 = idle)
    while True:
        await RisingEdge(dut.clk)
        await ReadOnly()  # inputs change on falling edges: these are what the DUT sampled
        if dut.rst.value == 1:
            model.reset()
            sb.expected.clear()  # reset cancels any update in progress
            in_flight = 0
            continue

        if dut.sample_valid.value == 1:
            if in_flight:
                sb.errors.append("sample_valid during an update (breaks the module's rules)")
            sb.expected.append(model.update(
                setpoint=dut.setpoint.value.to_signed(),
                measured=dut.measured.value.to_signed(),
                kp=dut.kp.value.to_signed(),
                ki=dut.ki.value.to_signed(),
                out_min=dut.out_min.value.to_signed(),
                out_max=dut.out_max.value.to_signed(),
            ))
            in_flight = LATENCY

        if dut.clear.value == 1:
            if in_flight and sb.checking:
                sb.errors.append("clear during an update: the model can't predict this")
            model.clear()

        if in_flight:
            in_flight -= 1


async def output_monitor(dut, sb):
    """Compare every output with the oldest expected result."""
    while True:
        await RisingEdge(dut.clk)
        await ReadOnly()
        if dut.rst.value == 1 or dut.out_valid.value != 1:
            continue
        sb.outputs += 1
        actual = (dut.out.value.to_signed(), int(dut.saturated.value) == 1)
        if not sb.expected:
            sb.errors.append(f"output {sb.outputs}: {actual} with no sample to match it")
            continue
        expected = sb.expected.popleft()
        if sb.checking and actual != expected:
            sb.errors.append(f"output {sb.outputs}: (out, saturated) = {actual}, expected {expected}")


def check_scoreboard(sb):
    shown = "\n".join(sb.errors[:10])
    assert not sb.errors, f"{len(sb.errors)} scoreboard errors:\n{shown}"
    assert not sb.expected, f"{len(sb.expected)} expected outputs never arrived"


async def setup(dut, checking=True):
    """Start the clock and both monitors, then reset. Returns the scoreboard."""
    Clock(dut.clk, 10, unit="ns").start()
    sb = Scoreboard(expected=deque(), errors=[], checking=checking)
    cocotb.start_soon(input_monitor(dut, PiModel(), sb))
    cocotb.start_soon(output_monitor(dut, sb))

    dut.rst.value = 1
    for name in ("clear", "sample_valid", "setpoint", "measured",
                 "kp", "ki", "out_min", "out_max"):
        getattr(dut, name).value = 0
    await ClockCycles(dut.clk, 3)
    await FallingEdge(dut.clk)  # change inputs while the clock is low
    dut.rst.value = 0
    return sb


# --- directed tests ---

@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def single_sample(dut):
    """One P-only sample: error 1.5 A x kp 2.0 = 3 cycles."""
    sb = await setup(dut)
    out, sat = await run_sample(dut, PiSample(setpoint=1536, measured=0, kp=2 * GAIN))
    assert (out, sat) == (3, 0), f"got out={out}, saturated={sat}"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def latency_is_four_cycles(dut):
    """out_valid is high 4 rising edges after sample_valid is driven, for one cycle only."""
    sb = await setup(dut)
    await drive_sample(dut, PiSample(setpoint=AMP, measured=0, kp=GAIN))
    # drive_sample returned one falling edge after raising sample_valid, so the
    # rising edge that sampled it has passed: count the remaining edges
    seen = []
    for _ in range(6):
        await RisingEdge(dut.clk)
        await ReadOnly()
        seen.append(int(dut.out_valid.value))
    # edges after the sampling edge: 1, 2, 3 -> out_valid; i.e. 4 including it
    assert seen == [0, 0, 1, 0, 0, 0], f"out_valid on edges after sampling: {seen}"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def reset_clears_everything(dut):
    """Reset mid-update: no output for that update, and the integrator restarts at 0."""
    sb = await setup(dut)
    for _ in range(3):  # build the integrator up to 3 cycles
        await run_sample(dut, PiSample(setpoint=AMP, measured=0, ki=GAIN))

    await drive_sample(dut, PiSample(setpoint=AMP, measured=0, ki=GAIN))
    await set_inputs(dut, rst=1)
    for _ in range(4):
        await RisingEdge(dut.clk)
        await ReadOnly()
        assert dut.out_valid.value == 0, "out_valid during reset"
        assert dut.out.value.to_signed() == 0, "out not cleared by reset"
        assert dut.saturated.value == 0, "saturated not cleared by reset"
    await set_inputs(dut, rst=0)
    await ClockCycles(dut.clk, 6)  # the cancelled update must not appear late

    out, _ = await run_sample(dut, PiSample(setpoint=AMP, measured=0, ki=GAIN))
    assert out == 1, f"integrator didn't restart from 0 after reset: out={out}"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def integral_ramps(dut):
    """kp = 0, constant 1 A error, ki 1.0: the output rises by 1 cycle per sample."""
    sb = await setup(dut)
    for n in range(1, 21):
        out, sat = await run_sample(dut, PiSample(setpoint=AMP, measured=0, ki=GAIN))
        assert (out, sat) == (n, 0), f"sample {n}: out={out}, saturated={sat}"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def saturates_both_ways(dut):
    """Outputs past a limit are clamped to it and flagged as saturated."""
    sb = await setup(dut)
    big_error = 10 * AMP  # x kp 2.0 = 20 cycles
    out, sat = await run_sample(dut, PiSample(big_error, 0, kp=2 * GAIN, out_min=-10, out_max=10))
    assert (out, sat) == (10, 1), f"high: out={out}, saturated={sat}"
    out, sat = await run_sample(dut, PiSample(0, big_error, kp=2 * GAIN, out_min=-10, out_max=10))
    assert (out, sat) == (-10, 1), f"low: out={out}, saturated={sat}"
    out, sat = await run_sample(dut, PiSample(AMP, 0, kp=2 * GAIN, out_min=-10, out_max=10))
    assert (out, sat) == (2, 0), f"back in range: out={out}, saturated={sat}"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def windup_recovery(dut):
    """After a long time saturated, reversing the error moves the output straight away."""
    sb = await setup(dut)
    push = PiSample(setpoint=20 * AMP, measured=0, kp=GAIN, ki=GAIN, out_min=0, out_max=100)
    for _ in range(50):
        await run_sample(dut, push)
        integrator = dut.integrator.value.to_signed()  # white-box check
        assert integrator <= 100 * CYCLE, f"integrator wound up to {integrator / CYCLE} cycles"

    out, _ = await run_sample(dut, PiSample(setpoint=0, measured=5 * AMP, kp=GAIN, ki=GAIN,
                                            out_min=0, out_max=100))
    # Without anti-windup the integrator would be ~1000 cycles and out would stay at 100
    assert out < 100, f"output still at the limit after the error reversed: out={out}"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def config_captured_on_sample_valid(dut):
    """Changing gains or limits after sample_valid doesn't affect that update."""
    sb = await setup(dut)
    for delay in range(3):  # change on each cycle of the update
        await drive_sample(dut, PiSample(setpoint=AMP, measured=0, kp=GAIN, out_min=-50, out_max=50))
        if delay:
            await ClockCycles(dut.clk, delay, rising=False)
        dut.kp.value = 7 * GAIN
        dut.out_min.value = 0
        dut.out_max.value = 1
        out, _ = await wait_for_output(dut)
        assert out == 1, f"change {delay} cycles after sample_valid leaked in: out={out}"
        # model check: input_monitor read the pins at sample_valid, so it expects 1 too
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def clear_timing(dut):
    """clear zeroes the integrator; its effect on an update in progress depends on timing.

    Expected values come from docs/pi_controller_vplan.md section 4 (T7). The model
    can't predict clear during an update, so the scoreboard isn't used here.
    """
    await setup(dut, checking=False)
    step = PiSample(setpoint=AMP, measured=0, ki=GAIN)  # +1 cycle per sample, kp = 0

    async def build_to_three():
        for _ in range(3):
            await run_sample(dut, step)

    # clear while idle: integrator 0, next sample gives 1
    await build_to_three()
    await set_inputs(dut, clear=1)
    await set_inputs(dut, clear=0)
    out, _ = await run_sample(dut, step)
    assert out == 1, f"clear while idle: out={out}, expected 1"

    # clear in cycle n+2 (sampled with the integrator update): out = p only = 0
    await set_inputs(dut, clear=1)
    await set_inputs(dut, clear=0)
    await build_to_three()
    await drive_sample(dut, step)          # now in cycle n+1
    result = cocotb.start_soon(wait_for_output(dut))  # watch from now on, so it can't be missed
    await set_inputs(dut, clear=1)         # cycle n+2
    await set_inputs(dut, clear=0)
    out, _ = await result
    assert out == 0, f"clear in cycle n+2: out={out}, expected 0 (p only)"

    # clear in cycle n+3: out still includes the integrator (4), but it's 0 afterwards
    await build_to_three()
    await drive_sample(dut, step)          # cycle n+1
    result = cocotb.start_soon(wait_for_output(dut))
    await FallingEdge(dut.clk)             # cycle n+2
    await set_inputs(dut, clear=1)         # cycle n+3
    await set_inputs(dut, clear=0)         # out_valid is already high by now
    out, _ = await result
    assert out == 4, f"clear in cycle n+3: out={out}, expected 4"
    out, _ = await run_sample(dut, step)
    assert out == 1, f"after clear in cycle n+3: out={out}, expected 1"


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def extremes(dut):
    """Largest errors and gains, every sign combination: nothing wraps."""
    sb = await setup(dut)
    for setpoint, measured in ((CURRENT_MAX, CURRENT_MIN), (CURRENT_MIN, CURRENT_MAX)):
        for kp, ki in ((GAIN_MAX, 0), (GAIN_MIN, 0), (0, GAIN_MAX), (0, GAIN_MIN),
                       (GAIN_MAX, GAIN_MIN), (GAIN_MIN, GAIN_MAX)):
            await set_inputs(dut, clear=1)  # start each case from integrator = 0
            await set_inputs(dut, clear=0)
            out, sat = await run_sample(dut, PiSample(setpoint, measured, kp, ki,
                                                      OUTPUT_MIN, OUTPUT_MAX))
            error_sign = 1 if setpoint > measured else -1
            if kp:
                # P alone is far past a limit: saturates in the direction of error x kp
                expected = (OUTPUT_MAX if error_sign * kp > 0 else OUTPUT_MIN, 1)
            else:
                # ki only: one increment is bigger than the whole output range, so the
                # skip rule rejects it and the output never moves (accepted design choice)
                expected = (0, 0)
            assert (out, sat) == expected, (
                f"error {setpoint - measured}, kp {kp}, ki {ki}: out={out}, saturated={sat}")
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def rounding_ties(dut):
    """Exact halves round up: +0.5 -> 1, -0.5 -> 0."""
    sb = await setup(dut)
    out, _ = await run_sample(dut, PiSample(AMP, 0, kp=GAIN // 2, out_min=-10, out_max=10))
    assert out == 1, f"+0.5 rounded to {out}, expected 1"
    out, _ = await run_sample(dut, PiSample(0, AMP, kp=GAIN // 2, out_min=-10, out_max=10))
    assert out == 0, f"-0.5 rounded to {out}, expected 0"
    check_scoreboard(sb)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def limit_cases(dut):
    """out_min == out_max, and limits tightened while the integrator is above them."""
    sb = await setup(dut)
    out, sat = await run_sample(dut, PiSample(AMP, 0, kp=GAIN, out_min=5, out_max=5))
    assert out == 5, f"out_min == out_max == 5: out={out}"

    await set_inputs(dut, clear=1)
    await set_inputs(dut, clear=0)
    for _ in range(5):  # integrator up to 50 cycles
        await run_sample(dut, PiSample(10 * AMP, 0, ki=GAIN, out_min=0, out_max=100))
    out, sat = await run_sample(dut, PiSample(10 * AMP, 0, ki=GAIN, out_min=0, out_max=20))
    assert (out, sat) == (20, 1), f"after tightening the limit: out={out}, saturated={sat}"
    check_scoreboard(sb)


# --- random regression ---

def random_current():
    if random.random() < 0.2:
        return random.choice([CURRENT_MAX, CURRENT_MIN, 0, 1, -1])
    return random.randint(-4 * AMP, 4 * AMP)


def random_config():
    """Gains and limits, steered towards realistic values with some extremes."""
    roll = random.random()
    if roll < 0.1:
        kp, ki = random.choice([GAIN_MAX, GAIN_MIN, 0]), random.choice([GAIN_MAX, GAIN_MIN, 0])
    else:
        kp, ki = random.randint(0, 4 * GAIN), random.randint(0, GAIN // 4)
    roll = random.random()
    if roll < 0.1:
        out_min = out_max = random.randint(-100, 100)
    elif roll < 0.4:
        out_min, out_max = 0, random.randint(1, 200)  # PWM-like: small, non-negative
    elif roll < 0.5:
        out_min, out_max = OUTPUT_MIN, OUTPUT_MAX
    else:
        out_min, out_max = sorted(random.randint(-500, 500) for _ in range(2))
    return dict(kp=kp, ki=ki, out_min=out_min, out_max=out_max)


@cocotb.test(timeout_time=TIMEOUT_US, timeout_unit="us")
async def random_regression(dut):
    """Random samples checked against the model. Re-run a failure with RANDOM_SEED=<seed>."""
    sb = await setup(dut)
    config = random_config()
    saturated = 0
    for _ in range(2000):
        roll = random.random()
        if roll < 0.02:
            config = random_config()  # new gains/limits; the integrator carries over
        elif roll < 0.03:
            await set_inputs(dut, clear=1)  # only between updates, where the model is exact
            await set_inputs(dut, clear=0)
        _, sat = await run_sample(dut, PiSample(random_current(), random_current(), **config))
        saturated += sat
    dut._log.info(f"2000 samples, {saturated} saturated, {sb.outputs} outputs checked")
    check_scoreboard(sb)


def test_pi_controller():
    run("pi_controller", "test_pi_controller")
