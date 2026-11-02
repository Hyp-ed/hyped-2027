"""Unit tests for the PI reference model (tb/models/pi_model.py).

The model is the judge for every hardware test, so it is checked here first
against values worked out by hand. No simulator needed.

All values are raw fixed-point integers, as the hardware sees them:
  current  Q6.10   1 A   = 1024
  gain     Q16.16  1.0   = 65536
  output   Q16.0   1     = 1 clock cycle
  integrator Q.16  1 cycle = 65536

Run with:  make model-tests   (or: python -m pytest tb/tests/test_pi_model.py)
"""

from models.pi_model import PiModel, round_shift

AMP = 1 << 10     # 1 A in Q6.10
GAIN = 1 << 16    # gain 1.0 in Q16.16
CYCLE = 1 << 16   # 1 output cycle in the integrator's Q.16

# Round shift - should round up

def test_round_shift_positive():
    assert round_shift(1536, 10) == 2    
    assert round_shift(1500, 10) == 1    
    assert round_shift(1024, 10) == 1    


def test_round_shift_negative():
    assert round_shift(-1536, 10) == -1  
    assert round_shift(-1537, 10) == -2  
    assert round_shift(-1024, 10) == -1  


def test_round_shift_zero_shift_is_unchanged():
    assert round_shift(-7, 0) == -7


# --- update ---

def test_p_only():
    # error 1.5 A x kp 2.0 = 3 cycles
    model = PiModel()
    assert model.update(setpoint=1536, measured=0, kp=2 * GAIN, ki=0,
                        out_min=0, out_max=1000) == (3, False)


def test_i_only_accumulates():
    # error 1 A x ki 0.5 adds 0.5 cycles per sample: integrator 0.5, 1.0, 1.5
    # out = round(integrator) = 1, 1, 2 (the 0.5 and 1.5 are ties, rounding up)
    model = PiModel()
    outputs = [model.update(setpoint=AMP, measured=0, kp=0, ki=GAIN // 2,
                            out_min=0, out_max=1000)
               for _ in range(3)]
    assert outputs == [(1, False), (1, False), (2, False)]
    assert model.integrator == 3 * CYCLE // 2


def test_saturates_high():
    # error 10 A x kp 2.0 = 20 cycles, limit 10
    model = PiModel()
    assert model.update(setpoint=10 * AMP, measured=0, kp=2 * GAIN, ki=0,
                        out_min=0, out_max=10) == (10, True)


def test_saturates_low():
    # error -10 A x kp 2.0 = -20 cycles, limit 0
    model = PiModel()
    assert model.update(setpoint=0, measured=10 * AMP, kp=2 * GAIN, ki=0,
                        out_min=0, out_max=10) == (0, True)


def test_output_rounding_ties():
    # kp 0.5 x error +/-1 A = +/-0.5 cycles exactly
    model = PiModel()
    assert model.update(setpoint=AMP, measured=0, kp=GAIN // 2, ki=0,
                        out_min=-10, out_max=10) == (1, False)    # +0.5 -> 1
    assert model.update(setpoint=0, measured=AMP, kp=GAIN // 2, ki=0,
                        out_min=-10, out_max=10) == (0, False)    # -0.5 -> 0, not -1


def test_anti_windup_skips_increment_while_saturated():
    # P alone (20 cycles) is past the limit (10), so every positive increment
    # is skipped and the integrator never winds up
    model = PiModel()
    for _ in range(5):
        assert model.update(setpoint=10 * AMP, measured=0, kp=2 * GAIN, ki=GAIN,
                            out_min=0, out_max=10) == (10, True)
    assert model.integrator == 0


def test_integrator_clamped_to_limits():
    # ki only: 4 cycles per sample, then a negative P term lets the increment
    # through, but the integrator itself must not exceed out_max (10 cycles)
    model = PiModel()
    model.update(setpoint=4 * AMP, measured=0, kp=0, ki=GAIN, out_min=0, out_max=10)
    model.update(setpoint=4 * AMP, measured=0, kp=0, ki=GAIN, out_min=0, out_max=10)
    assert model.integrator == 8 * CYCLE
    # error +4 A, kp -2.0 -> p = -8; candidate integrator 12, test output 4: allowed
    model.update(setpoint=4 * AMP, measured=0, kp=-2 * GAIN, ki=GAIN, out_min=0, out_max=10)
    assert model.integrator == 10 * CYCLE    # 12 clamped to 10


def test_clear_and_reset_zero_the_integrator():
    model = PiModel()
    model.update(setpoint=AMP, measured=0, kp=0, ki=GAIN, out_min=0, out_max=100)
    model.clear()
    assert model.integrator == 0
    model.update(setpoint=AMP, measured=0, kp=0, ki=GAIN, out_min=0, out_max=100)
    model.reset()
    assert model.integrator == 0


def test_increment_rejected_if_it_would_overshoot():
    # Design choice (2026-10-29): an increment that would take the output past
    # a limit is skipped entirely, not partly applied. So the output can settle
    # up to one increment short of the limit. Accepted because real increments
    # (ki x T) are tiny. If the rule changes, this test must change with it.
    model = PiModel()
    outputs = [model.update(setpoint=4 * AMP, measured=0, kp=0, ki=GAIN,
                            out_min=0, out_max=10)
               for _ in range(4)]
    assert outputs == [(4, False), (8, False), (8, False), (8, False)]
