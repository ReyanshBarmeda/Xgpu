# Timing and I/O Constraints (SDC) for XGPU Top-Level Module (`gpu`)
# Target: SKY130 (sky130_fd_sc_hd) initial digital RTL-to-GDSII experimentation
#
# Top-level parameters (default):
#   NUM_CORES = 2, THREADS_PER_BLOCK = 4
#   DATA_MEM_ADDR_BITS = 8, DATA_MEM_DATA_BITS = 8, DATA_MEM_NUM_CHANNELS = 4
#   PROGRAM_MEM_ADDR_BITS = 8, PROGRAM_MEM_DATA_BITS = 16, PROGRAM_MEM_NUM_CHANNELS = 1
#   Total signal I/O ports: 183 (1 clk, 1 reset, 11 host control, 26 program mem, 144 data mem)
#
# Rationale for 40 MHz (25.0 ns) initial clock period:
#   src/alu.sv instantiates single-cycle combinational 8-bit integer division (`rs / rt`)
#   and multiplication (`rs * rt`) per thread (8 instances total). A 25.0 ns period provides
#   a realistic starting target for SKY130 timing closure before any RTL pipelining.

set clk_name   core_clk
set clk_port   clk
set clk_period 25.0
set clk_io_pct 0.20

# Primary clock definition
create_clock -name $clk_name -period $clk_period [get_ports $clk_port]

# Realistic clock uncertainty (jitter + pre-CTS skew allowance) and transition
set_clock_uncertainty 0.25 [get_clocks $clk_name]
set_clock_transition  0.15 [get_clocks $clk_name]

# Reset path: treat external reset assertion as asynchronous/false-path for initial bring-up
if {[llength [get_ports reset]] > 0} {
    set_false_path -from [get_ports reset]
}

# I/O delay budgets (20% max delay, 4% min delay of clock period)
set input_delay_max  [expr {$clk_period * $clk_io_pct}]
set input_delay_min  [expr {$clk_period * 0.04}]
set output_delay_max [expr {$clk_period * $clk_io_pct}]
set output_delay_min [expr {$clk_period * 0.04}]

# OpenSTA does not implement Synopsys' remove_from_collection command.
# List only actual top-level input ports (excluding clk) using port-name patterns.
set non_clk_inputs [get_ports {reset start device_control_write_enable device_control_data* program_mem_read_ready* program_mem_read_data* data_mem_read_ready* data_mem_read_data* data_mem_write_ready*}]

set_input_delay  -max $input_delay_max  -clock $clk_name $non_clk_inputs
set_input_delay  -min $input_delay_min  -clock $clk_name $non_clk_inputs
set_output_delay -max $output_delay_max -clock $clk_name [all_outputs]
set_output_delay -min $output_delay_min -clock $clk_name [all_outputs]

# Design rule constraints (SKY130 HD standard defaults)
set_max_fanout 10 [current_design]
set_max_transition 0.75 [current_design]

# Output capacitive load (33.4 fF typical SKY130 digital macro load)
# NOTE: Does NOT model package bondwire/ball capacitance or GDDR5 high-speed I/O.
set_load 0.0334 [all_outputs]

# Input driving cell (guarded so SDC remains portable across OpenROAD / OpenLane 2)
if {[llength [get_lib_cells */sky130_fd_sc_hd__inv_2]] > 0} {
    set_driving_cell -lib_cell sky130_fd_sc_hd__inv_2 -pin Y $non_clk_inputs
}
