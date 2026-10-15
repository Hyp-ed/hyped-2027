"""Shared helper for cocotb testbenches.

Builds a module with Verilator and runs the cocotb tests in a tb/test_<name>.py
file. Each test file calls run() from a small pytest function, so the same
file holds both the tests and the instructions for running them.

Run a module's cocotb tests with:  make cocotb TOP=<name>
"""

import os
from pathlib import Path

from cocotb_tools.runner import get_runner

HDL_DIR = Path(__file__).resolve().parent.parent

# Set by the Makefile. Verilator's generated makefiles can't cope with spaces in
# paths, so the Makefile passes a build folder and a route to hdl/ without any.
# abspath (not resolve) keeps the space-free symlink path intact.
BUILD_DIR = Path(os.path.abspath(os.environ.get("HDL_BUILD", HDL_DIR / "build")))
SRC_DIR = Path(os.path.abspath(os.environ.get("HDL_SRC", HDL_DIR)))


def run(toplevel, test_module, parameters=None, waves=True):
    """Build rtl/<toplevel>.v with the given parameters, then run test_module's tests."""
    parameters = parameters or {}

    # A separate build per parameter set, e.g. build/cocotb/pwm_WIDTH4
    suffix = "_".join(f"{name}{value}" for name, value in sorted(parameters.items()))
    build_dir = BUILD_DIR / "cocotb" / (f"{toplevel}_{suffix}" if suffix else toplevel)

    runner = get_runner("verilator")
    runner.build(
        sources=[SRC_DIR / "rtl" / f"{toplevel}.v"],
        hdl_toplevel=toplevel,
        build_args=["-y", str(SRC_DIR / "rtl")],  # find submodules in rtl/
        parameters=parameters,
        build_dir=build_dir,
        waves=waves,
    )
    runner.test(
        hdl_toplevel=toplevel,
        test_module=test_module,
        build_dir=build_dir,
        waves=waves,
        # Tests read their build's parameters from PARAM_<NAME> variables
        extra_env={f"PARAM_{name}": str(value) for name, value in parameters.items()},
    )
