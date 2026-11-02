"""

This model replicates the Pi Controller in python and is used 
as a transaction-level reference model for testbenches to use.

"""

INT_FRAC = 16
PROD_FRAC = 26          # CURRENT_FRAC (10) + GAIN_FRAC (16)

class PiModel:
    def __init__(self):
        self.integrator = 0                     # Q.16

    def update(self, setpoint, measured, kp, ki, out_min, out_max):

        error = setpoint - measured

        p_prod = error * kp
        i_prod = error * ki

        p_term    = round_shift(p_prod, PROD_FRAC - INT_FRAC)
        increment = round_shift(i_prod, PROD_FRAC - INT_FRAC)
        hi = out_max << INT_FRAC
        lo = out_min << INT_FRAC
        candidate = self.integrator + increment
        test_out  = p_term + candidate

        applied = not (test_out > hi and increment > 0) and not (test_out < lo and increment < 0)
        if applied:
            self.integrator = clamp(candidate, lo, hi)

        rounded   = round_shift(p_term + self.integrator, INT_FRAC)
        out       = clamp(rounded, out_min, out_max)
        saturated = rounded != out
        return out, saturated

    def reset(self):
        self.integrator = 0

    def clear(self):
        self.integrator = 0


def round_shift(x, s):
    if s <= 0:
        return x
    return (x + (1 << (s - 1))) >> s

def clamp(x, min_val, max_val):
    if(x < min_val):
        return min_val
    elif (x > max_val):
        return max_val
    else:
        return x
    
