# Build and run Verilator simulations.
#
#   make          build and run the testbench (same as `make sim`)
#   make lint     check the Verilog for mistakes without simulating
#   make waves    open the waveform in Surfer
#   make clean    delete everything generated
#
# Run these from inside the hdl/ folder.

TOP := pwm
RTL := rtl/pwm.v
TB  := tb/tb_pwm.cpp

# Verilator's generated makefiles refuse to build in a path containing spaces.
# If this folder's path has one, build under ~/.cache instead and reach the
# sources through a symlink, so no path Verilator sees contains a space.
ifneq ($(word 2,$(CURDIR)),)
BUILD := $(HOME)/.cache/hyped-hdl
SRC   := $(BUILD)/src
else
BUILD := build
SRC   := .
endif

VERILATOR       := verilator
VERILATOR_FLAGS := -Wall --trace --cc --exe --build -j 0 --Mdir $(BUILD)/obj_dir

.PHONY: all sim lint waves clean

all: sim

$(BUILD)/obj_dir/V$(TOP): $(RTL) $(TB)
	@mkdir -p $(BUILD)
	@if [ "$(SRC)" != "." ]; then ln -sfn "$(CURDIR)" "$(SRC)"; fi
	$(VERILATOR) $(VERILATOR_FLAGS) --top-module $(TOP) $(SRC)/$(RTL) $(SRC)/$(TB)

# Run from inside the build folder so waves.vcd is written there
sim: $(BUILD)/obj_dir/V$(TOP)
	cd $(BUILD) && ./obj_dir/V$(TOP)

lint:
	$(VERILATOR) --lint-only -Wall --top-module $(TOP) $(RTL)

waves:
	surfer $(BUILD)/waves.vcd

clean:
	rm -rf $(BUILD)
