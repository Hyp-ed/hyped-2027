# Build, lint and test the HDL modules with Verilator.
#
#   make                     lint every module, then run every testbench
#   make sim   TOP=<name>    build and run one module's C++ testbench
#   make lint  TOP=<name>    lint one module
#   make waves TOP=<name>    open one module's C++ testbench waveform in Surfer
#   make cocotb TOP=<name>   run one module's cocotb (Python) testbench
#   make test-all            run every testbench (C++ and cocotb), report which failed
#   make lint-all            lint every module in rtl/
#   make list                show the modules and testbenches found
#   make clean               delete everything generated
#   make setup               create the Python environment for cocotb (once)
#
# Conventions this relies on (see docs/conventions.md):
#   - one module per file, and the file is named after the module: rtl/<name>.v
#     or rtl/<name>.sv (SystemVerilog)
#   - SystemVerilog packages are named rtl/<name>_pkg.sv; every package is
#     compiled ahead of the module, so any module can import any package
#   - a module's testbench is tb/tb_<name>.cpp (C++) or tb/test_<name>.py (cocotb)
# Submodules are found automatically in rtl/ (Verilator's -y option).
#
# Run from inside the hdl/ folder.

TOP ?= pwm

PKGS    := $(sort $(wildcard rtl/*_pkg.sv))
MODULES := $(sort $(basename $(notdir $(filter-out $(PKGS),$(wildcard rtl/*.v rtl/*.sv)))))
TESTS   := $(patsubst tb/tb_%.cpp,%,$(wildcard tb/tb_*.cpp))
COCOTB  := $(patsubst tb/test_%.py,%,$(wildcard tb/test_*.py))

# Use the project's Python environment if `make setup` has created it
PYTHON := $(if $(wildcard .venv/bin/python),.venv/bin/python,python3)

# Verilator's generated makefiles refuse to build in a path containing spaces.
# If this folder's path has one, build under ~/.cache instead (one folder per
# checkout) and reach the sources through a symlink with no spaces in it.
ifneq ($(word 2,$(CURDIR)),)
BUILD := $(HOME)/.cache/hyped-hdl/$(shell printf '%s' "$(CURDIR)" | cksum | cut -d ' ' -f 1)
SRC   := $(BUILD)/src
else
BUILD := build
SRC   := .
endif

VERILATOR  := verilator
LINT_FLAGS := -Wall -y $(SRC)/rtl
SIM_FLAGS  := $(LINT_FLAGS) --trace --cc --exe --build -j 0

RTL_FILES := $(wildcard rtl/*.v rtl/*.sv)
TOP_FILE  := $(firstword $(wildcard rtl/$(TOP).sv rtl/$(TOP).v))
TOP_SRCS  := $(addprefix $(SRC)/,$(PKGS) $(TOP_FILE))
TOP_DIR   := $(BUILD)/$(TOP)
TOP_EXE   := $(TOP_DIR)/obj_dir/V$(TOP)

.PHONY: all sim lint waves cocotb test-all lint-all list clean setup check-top

all: lint-all test-all

check-top:
	@test -n "$(TOP_FILE)" || { echo "No rtl/$(TOP).v or rtl/$(TOP).sv. Modules: $(MODULES)"; exit 1; }

$(TOP_EXE): $(RTL_FILES) tb/tb_$(TOP).cpp
	@mkdir -p $(TOP_DIR)
	@if [ "$(SRC)" != "." ]; then ln -sfn "$(CURDIR)" "$(SRC)"; fi
	$(VERILATOR) $(SIM_FLAGS) --Mdir $(TOP_DIR)/obj_dir --top-module $(TOP) \
		$(TOP_SRCS) $(SRC)/tb/tb_$(TOP).cpp

# Run from inside the module's build folder so waves.vcd is written there
sim: check-top $(TOP_EXE)
	cd $(TOP_DIR) && ./obj_dir/V$(TOP)

lint: check-top
	@mkdir -p $(BUILD)
	@if [ "$(SRC)" != "." ]; then ln -sfn "$(CURDIR)" "$(SRC)"; fi
	$(VERILATOR) --lint-only $(LINT_FLAGS) --top-module $(TOP) $(TOP_SRCS)

waves: check-top
	surfer $(TOP_DIR)/waves.vcd

# Waveforms land in $(BUILD)/cocotb/<name>_<parameters>/dump.vcd
cocotb: check-top
	@test -f tb/test_$(TOP).py || { echo "No tb/test_$(TOP).py. cocotb testbenches: $(COCOTB)"; exit 1; }
	@mkdir -p $(BUILD)
	@if [ "$(SRC)" != "." ]; then ln -sfn "$(CURDIR)" "$(SRC)"; fi
	HDL_BUILD="$(BUILD)" HDL_SRC="$(SRC)" $(PYTHON) -m pytest tb/test_$(TOP).py

test-all:
	@failed=""; \
	for t in $(TESTS); do \
		echo "=== $$t (C++)"; \
		$(MAKE) --no-print-directory sim TOP=$$t || failed="$$failed $$t"; \
	done; \
	for t in $(COCOTB); do \
		echo "=== $$t (cocotb)"; \
		$(MAKE) --no-print-directory cocotb TOP=$$t || failed="$$failed $$t(cocotb)"; \
	done; \
	echo; \
	if [ -n "$$failed" ]; then echo "FAILED:$$failed"; exit 1; fi; \
	echo "All testbenches passed ($(words $(TESTS)) C++, $(words $(COCOTB)) cocotb)."

lint-all:
	@failed=""; \
	for m in $(MODULES); do \
		$(MAKE) --no-print-directory lint TOP=$$m > /dev/null 2>&1 || failed="$$failed $$m"; \
	done; \
	if [ -n "$$failed" ]; then \
		echo "Lint FAILED:$$failed (run 'make lint TOP=<name>' to see why)"; exit 1; \
	fi; \
	echo "All modules lint clean ($(words $(MODULES)))."

list:
	@echo "Modules:     $(MODULES)"
	@echo "Packages:    $(notdir $(PKGS))"
	@echo "C++ tests:   $(TESTS)"
	@echo "cocotb:      $(COCOTB)"
	@echo "Python:      $(PYTHON)"
	@echo "Build dir:   $(BUILD)"

clean:
	rm -rf $(BUILD)

setup:
	python3 -m venv .venv
	.venv/bin/pip install --upgrade pip
	.venv/bin/pip install -r requirements.txt
