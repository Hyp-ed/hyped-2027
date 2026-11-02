"""Python reference models of the RTL modules.

Each model predicts what its module should output, cycle by cycle or update by
update, and the cocotb testbenches compare the hardware against it. Import with
e.g.  from models.pwm_model import PwmModel
"""
