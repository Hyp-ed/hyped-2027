# Verification plan: `pi_controller`

Status: draft (2026-10-29). DUT: `rtl/pi_controller.sv`, uses `rtl/fixed_point_pkg.sv`.

> **Concept: verification plan.** Before writing any test, list what the design
> must do (features), how each one will be checked, and how we will know we
> have tested enough (coverage). Tests are then written *from the plan*, not
> from the RTL, so the tests do not simply copy the RTL's mistakes.

## 1. Features to verify

| ID | Feature | Expected behaviour |
|---|---|---|
| F1 | Reset | After `rst`: `out = 0`, `out_valid = 0`, `saturated = 0`, integrator = 0 |
| F2 | Handshake and latency | If `sample_valid` is high in cycle n, `out_valid` is high in cycle n+4, for exactly one cycle |
| F3 | Error | `error = setpoint - measured`, 17 bits, never wraps |
| F4 | Proportional term | `p = round(kp * error)`, Q.26 → Q.16 |
| F5 | Integral term | `integrator += round(ki * error)`, Q.26 → Q.16; the output includes this sample's increment |
| F6 | Output | `out = clamp(round(p + integrator), out_min, out_max)`, Q.16 → Q16.0 |
| F7 | `saturated` | High when the output was clamped (rounded value outside the limits) |
| F8 | Anti-windup: conditional integration | Skip the increment if it would push further into saturation |
| F9 | Anti-windup: integrator clamp | The integrator stays within `[out_min, out_max]` (in Q.16) |
| F10 | `clear` | While high, the integrator is held at 0 |
| F11 | Captured configuration | `kp`, `ki`, `out_min` and `out_max` are captured on `sample_valid`; later changes don't affect that update |
| F12 | Rounding | Round to nearest, with halves rounding **up** (towards +∞), for negative values too |

## 2. Reference model

> **Concept: reference model, also called a golden model.** An independent
> model of what the DUT *should* output. The testbench compares the DUT
> against it, so random tests check themselves.

`tb/models/pi_model.py` is a Python model that matches the DUT bit for bit and works
one transaction at a time (one call per sample). Python integers have no
size limit, so the model cannot overflow:

```
round_shift(x, s) = (x + (1 << (s-1))) >> s      # matches the RTL's guard-bit rounding
                                                 # (>> is a floor shift in Python)
error     = setpoint - measured
p         = round_shift(error * kp, 10)          # Q.26 -> Q.16
inc       = round_shift(error * ki, 10)
hi, lo    = out_max << 16, out_min << 16
cand      = integ + inc
test      = p + cand
if not ((test > hi and inc > 0) or (test < lo and inc < 0)):
    integ = clamp(cand, lo, hi)
rounded   = round_shift(p + integ, 16)           # Q.16 -> Q.0
out       = clamp(rounded, out_min, out_max)
saturated = rounded != out
```

**Limitation:** this model doesn't track individual clock cycles, so it can't
predict what happens when `clear` changes while an update is in progress
(see T7). Random tests change `clear` only between updates, and directed tests
cover the in-progress timing.

> **Concept: transaction-level vs cycle-accurate models.** A transaction-level
> model is simpler and is enough for most checks. Timing details are checked
> separately, with assertions and directed tests.

## 3. Testbench structure

> **Concept: the layered testbench (UVM's structure).** Separate *what* to
> send (the sequence), *how* to drive the pins (the driver), *what was observed*
> (the monitor) and *whether it was right* (the scoreboard). Each part can
> then be reused and replaced on its own.

| Part | UVM name | Job |
|---|---|---|
| Sample generator | sequence | Produces (setpoint, measured, kp, ki, limits, clear) transactions, either directed or random |
| Driver | driver | Drives one transaction onto the pins and pulses `sample_valid`; waits at least 4 cycles before the next one |
| Input monitor | monitor | Records each transaction actually seen on `sample_valid` |
| Output monitor | monitor | Records (`out`, `saturated`) on each `out_valid` |
| Model and scoreboard | scoreboard | Runs each input through `pi_model.py` and compares it with the next output |
| Coverage | coverage collector | Counts the cases in section 6 that were hit |

## 4. Tests

| Test | Covers | Description |
|---|---|---|
| T1 reset | F1 | Check every output after reset, including after a reset in the middle of an update |
| T2 latency | F2 | One sample: `out_valid` is high exactly 4 cycles later, for exactly one cycle |
| T3 P only | F4, F6 | `ki = 0`, several errors of both signs, checked against values worked out by hand |
| T4 I only | F5 | `kp = 0` with a constant error: the output ramps up linearly, one increment per sample |
| T5 saturation | F6, F7 | Drive high and low past the limits; `out` equals the limit and `saturated = 1` |
| T6 windup recovery | F8, F9 | Hold in saturation for many samples, then reverse the error. Because the integrator was held at the limit instead of winding up past it, the output must start falling on the first sample after the reversal (when the error is large enough to move it at least half a cycle). **This test separates working anti-windup from broken anti-windup.** |
| T7 clear timing | F10 | `clear` while idle; `clear` in each cycle of an update in progress; release, then the next sample starts from integrator = 0 (expected values below) |
| T8 config capture | F11 | Change `kp`, `ki` or the limits in each of the 3 cycles after `sample_valid`; the result must use the captured values |
| T9 extremes | F3, F4 | Setpoint = +max and measured = −max (and the reverse); largest gains; every sign combination. Nothing wraps. |
| T10 rounding ties | F12 | Inputs chosen so p, the increment and the output each land exactly on .5, positive and negative |
| T11 limit cases | F6, F9 | `out_min == out_max`; `out_min = 0` (PWM use); limits changed between samples while the integrator is outside the new range |
| T12 random | all | Thousands of random samples, scoreboard against the model, seed recorded in the log, run with several seeds |

> **Concept: directed vs constrained-random tests.** Directed tests hit cases
> you thought of. Random tests find the ones you didn't. "Constrained" means
> the random values are steered towards interesting regions (limits, zero,
> extremes) instead of being uniformly spread.

Expected results for T7, if `sample_valid` is in cycle n (the integrator updates at the end of n+2, and the output is computed at the end of n+3):

| `clear` high in cycle | Effect |
|---|---|
| n+2 or earlier | The integrator is 0 after the update, so `out` = p only |
| n+3 | `out` still includes the old integrator; the integrator is 0 afterwards |

## 5. Assertions

> **Concept: assertions.** Rules that hold on every cycle of every test,
> checked automatically. **Immediate** assertions check one moment.
> **Concurrent** (SVA) assertions check sequences over time, e.g.
> `sample_valid |-> ##4 out_valid`. Verilator supports a subset of SVA;
> check each one compiles.

| ID | Rule |
|---|---|
| A1 | `sample_valid` in cycle n ⇒ `out_valid` in cycle n+4, and `out_valid` is never high for two cycles in a row |
| A2 | `out_valid` ⇒ `out_min_q <= out <= out_max_q` |
| A3 | `saturated` ⇒ `out == out_min_q` or `out == out_max_q` |
| A4 | No `sample_valid` while an update is in progress (`valid_0`, `valid_1` or `valid_2` high). This is a rule the *user* of the module must follow, not a DUT check. |
| A5 | `clear` in cycle n ⇒ integrator == 0 in cycle n+1 |

## 6. Functional coverage

> **Concept: code vs functional coverage.** Code coverage shows which lines
> ran. Functional coverage shows which *situations* were tested, defined
> from the plan. 100% code coverage can still miss whole situations.

| Item | Bins |
|---|---|
| error | large negative, small negative, 0, small positive, large positive, +max, −max |
| output | within range, saturated high, saturated low |
| anti-windup | increment applied, skipped (high), skipped (low), integrator clamped high, integrator clamped low |
| clear | idle, in each cycle of an update in progress, just released |
| gains | 0, small, large, negative (if allowed, see Q2) |
| rounding | tie in p, increment or output, positive and negative |
| limits | `min == max`, `min = 0`, changed between samples |
| cross | saturated-high × error going negative (and the reverse): the recovery case |

Done when: all tests pass, every coverage bin is hit, and T12 passes on at
least 10 seeds.

## 7. Open questions (to resolve before or during verification)

- **Q1** `output_t` is signed Q16.0 (±32767), but `conventions.md` says PWM duty is
  *unsigned* 0–65535. Which is right? Is the plan to use `out_min = 0`?
- **Q2** Are negative gains allowed? `gain_t` is signed, so the hardware accepts them.
- **Q3** The documented rule is one update at a time. Is that enough for the
  planned sample rate? It almost certainly is (100 MHz clock vs a kHz-range loop).
- **Q4** T7 shows the effect of `clear` depends on which pipeline cycle it
  lands in. Is that acceptable, given `clear` comes from the power-stage
  enable?
- **Q5** If the limits are tightened between samples, the integrator can sit
  outside the new range until an update is allowed through. Is that intended?
- **Q6** Halves always rounding up adds a tiny positive bias. That's probably
  negligible for control, but it should be a deliberate choice.
