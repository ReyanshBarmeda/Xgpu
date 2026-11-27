# OpenROAD-flow-scripts (ORFS) Design Configuration for XGPU
# Target Platform: SKY130 High-Density (sky130hd) — digital-flow experimentation only.
# NOTE: SKY130 digital flow does NOT implement a GDDR5 PHY. See physical_design/README.md.
#
# Prerequisite:
#   Run `./physical_design/prepare_rtl.sh` first to generate `physical_design/build/gpu_sv2v.v`.
#
# Usage from OpenROAD-flow-scripts/flow:
#   make DESIGN_CONFIG=/path/to/Xgpu/physical_design/config.mk

export DESIGN_NAME = gpu
export PLATFORM    = sky130hd

# Resolve paths relative to this config.mk file
XGPU_PD_DIR := $(abspath $(dir $(lastword $(MAKEFILE_LIST))))

# Converted Verilog-2005 netlist containing all 12 src/*.sv modules
export VERILOG_FILES = $(XGPU_PD_DIR)/build/gpu_sv2v.v

# Timing and I/O constraints (40 MHz initial target on SKY130)
export SDC_FILE      = $(XGPU_PD_DIR)/constraint.sdc

# Floorplan parameters (explicit, conservative for 183 signal I/O pins + 8 combinational dividers)
export CORE_UTILIZATION  = 35
export CORE_ASPECT_RATIO = 1
export CORE_MARGIN       = 10

# Placement & routing parameters
export PLACE_DENSITY     = 0.55
export TNS_END_PERCENT   = 100

# Synthesis options (area-aware ABC mapping for experimental SKY130 implementation)
export ABC_AREA          = 1
export SYNTH_HIERARCHICAL = 0

# Optional pin placement constraints (OpenLane 2 uses pin_order.cfg directly;
# for ORFS, custom IO_CONSTRAINTS Tcl can be supplied if needed)
export PLACE_PINS_ARGS   = -min_distance 2 -min_distance_in_tracks
