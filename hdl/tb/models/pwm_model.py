class PwmModel:
    """Reference model of pwm.v, following the spec in its header.

    clock() is called once per rising edge with the inputs as they were just
    before the edge, and returns what (pwm_out, centre_pulse) should be just
    after it.
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
        self.centre = 0

    def clock(self, rst, enable, duty):
        if rst:
            self.reset()
            return self.out, self.centre

        in_pulse = self.start <= self.counter < self.start + self.duty
        self.out = int(enable and self.enabled and in_pulse)
        # Middle of the period, every period, whatever the duty or enable
        self.centre = int(self.counter == (self.count - 1) // 2)

        if self.counter == self.count - 1:  # end of period: take new settings
            self.duty = duty
            self.start = (self.count - duty) // 2 if duty < self.count else 0
            self.enabled = bool(enable)
            self.counter = 0
        else:
            self.counter += 1
        return self.out, self.centre
