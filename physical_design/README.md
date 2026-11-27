# XGPU Physical Design (SKY130 / Yosys / OpenROAD / OpenLane 2)

This directory configures an initial, reproducible digital RTL-to-GDSII physical-design flow for the [Xgpu](https://github.com/ReyanshBarmeda/Xgpu) top-level module (`gpu` in [`src/gpu.sv`](https://github.com/ReyanshBarmeda/Xgpu/blob/main/src/gpu.sv)) targeting the open-source **SKY130** (`sky130A` / `sky130_fd_sc_hd`) PDK.

> **Status:** Configuration and CI workflows are prepared for reproducible execution. No synthesis area/cell-count numbers or physical-design results are fabricated here; run the commands below (or trigger `.github/workflows/openroad.yml`) to generate actual reports and GDSII. **XGPU is not tapeout-ready** (see [Blockers Before Fabrication](#blockers-before-fabrication)).

## Files in `physical_design/`

| File | Purpose |
| :--- | :--- |
| `prepare_rtl.sh` | Converts all 12 `src/*.sv` modules into Verilog-2005 (`physical_design/build/gpu_sv2v.v`) via `sv2v`, avoiding the root `Makefile`'s `-I src/*` argument bug. |
| `config.mk` | [OpenROAD-flow-scripts (ORFS)](https://openroad-flow-scripts.readthedocs.io/en/latest/user/FlowVariables.html) design configuration targeting `sky130hd`. |
| `config.json` | [OpenLane 2](https://github.com/chipfoundry/openlane2) (`Classic` flow) configuration targeting `sky130A` / `sky130_fd_sc_hd`. |
| `constraint.sdc` | Realistic timing and I/O constraints (40 MHz / 25.0 ns initial clock target, I/O delays, max fanout/transition, capacitive load). |
| `pin_order.cfg` | Die-edge pin placement grouping all 183 top-level signal pins across North (control/clk/rst), West (program memory), East (data read), and South (data write). |
| `synth.ys` | Standalone Yosys synthesis script producing cell counts (`reports/synth_stat.txt`) and gate-level netlist. |
| `patches/rtl_hygiene.patch` | Optional minimal hygiene diff fixing the trailing comma in `src/dcr.sv` and the `sv2v -I src/*` bug in `Makefile` without changing GPU architecture. |

## Top-Level Architecture & Port Summary (Unchanged)

The existing architecture in [`src/gpu.sv`](https://github.com/ReyanshBarmeda/Xgpu/blob/main/src/gpu.sv) is preserved without reducing cores, altering ISA behavior, inventing SRAM macros, or adding a fake GDDR5 PHY:

- **Top module:** `gpu`
- **Default parameters:** `NUM_CORES = 2`, `THREADS_PER_BLOCK = 4`, `DATA_MEM_ADDR_BITS = 8`, `DATA_MEM_DATA_BITS = 8`, `DATA_MEM_NUM_CHANNELS = 4`, `PROGRAM_MEM_ADDR_BITS = 8`, `PROGRAM_MEM_DATA_BITS = 16`, `PROGRAM_MEM_NUM_CHANNELS = 1`
- **Total signal I/O pins:** **183** (13 host control/clk/reset + 26 program memory + 144 data memory)

### External Memory Interfaces (Current RTL)

The current RTL exposes simple synchronous valid/ready parallel buses (`program_mem_*` and `data_mem_*`) with 8-bit addresses (256-entry address space per interface). These are **behavioral/digital memory-controller interfaces**, not a GDDR5 bus.

## Important GDDR5 Architectural Limitation

The future XGPU system targets **2 GB of external GDDR5 memory**. A standard digital OpenROAD/OpenLane 2 flow on SKY130 **does not and cannot produce a functional GDDR5 PHY**. Four distinct engineering layers must be kept separate:

1. **GDDR5 Memory Controller RTL (Digital):** Handles bank/row/column scheduling, refresh timers, command queues, and protocol state machines. This layer *can* be written in synthesizable RTL (and does not exist in XGPU yet—the current `controller.sv` is a simple multi-consumer valid/ready arbiter for 8-bit addresses).
2. **GDDR5 PHY (High-Speed Mixed-Signal / Analog Hard IP):** Requires multi-Gbps differential signaling, DQ/WCK/CK phase-locked loops (PLLs/DLLs), write/read leveling, ZQ impedance calibration, ODT, and PVT-compensated high-speed transceivers. **SKY130 (130 nm CMOS) cannot meet GDDR5 multi-Gbps signaling rates or provide a foundry-qualified GDDR5 PHY.**
3. **Physical I/O & Package Requirements:** GDDR5 requires high-pin-count flip-chip BGA packaging with controlled-impedance signal integrity, dedicated VDDQ/VSSQ power-delivery planes, and ESD-protected high-speed I/O pad rings—far beyond SKY130 wirebond packages.
4. **External Memory Chips & PCB:** Requires GDDR5 SDRAM devices, length-matched impedance-controlled differential/single-ended PCB traces, termination networks, and multi-layer PDN design.

## Legacy GDS Files (`gds/0/gpu.gds`, `gds/1/gpu.gds`)

The repository contains `gds/0/gpu.gds` and `gds/1/gpu.gds`, but includes no accompanying DRC/LVS signoff logs, SPEF/STA timing reports, PDK version records, or flow configurations. Treat these files as **unverified historical artifacts**, not fabrication-ready layouts.

## Exact Reproduction Commands

### 1. Prepare Verilog-2005 RTL (`sv2v`)

```bash
chmod +x physical_design/prepare_rtl.sh
./physical_design/prepare_rtl.sh
```

### 2. Standalone Yosys Synthesis (Cell Count & Optional SKY130 Area)

```bash
mkdir -p physical_design/reports
yosys -s physical_design/synth.ys | tee physical_design/reports/yosys_synth.log
```

To map to SKY130 HD Liberty for µm² area estimation (requires `sky130A` installed via Volare):

```bash
export SKY130_HD_LIB="$HOME/.volare/sky130A/libs.ref/sky130_fd_sc_hd/lib/sky130_fd_sc_hd__tt_025C_1v80.lib"
yosys -p "
  read_verilog physical_design/build/gpu_sv2v.v;
  hierarchy -check -top gpu;
  synth -top gpu;
  dfflibmap -liberty $SKY130_HD_LIB;
  abc -liberty $SKY130_HD_LIB;
  clean;
  stat -liberty $SKY130_HD_LIB;
  write_verilog -noattr physical_design/build/gpu_sky130_netlist.v
" | tee physical_design/reports/yosys_sky130.log
```

### 3. Full RTL-to-GDSII via OpenLane 2 (Recommended Reproducible Flow)

Using Docker ([OpenLane 2 documentation](https://openlane2.readthedocs.io/en/latest/getting_started/migrants/index.html)):

```bash
python3 -m pip install --upgrade "openlane==2.2.9"
./physical_design/prepare_rtl.sh
openlane --dockerized --pdk sky130A physical_design/config.json
```

Using Nix (Linux/macOS):

```bash
./physical_design/prepare_rtl.sh
nix run github:chipfoundry/openlane2 -- --pdk sky130A physical_design/config.json
```

### 4. Full RTL-to-GDSII via OpenROAD-flow-scripts (ORFS)

From a cloned [`OpenROAD-flow-scripts/flow`](https://openroad-flow-scripts.readthedocs.io/en/latest/tutorials/FlowTutorial.html) directory:

```bash
./path/to/Xgpu/physical_design/prepare_rtl.sh
make DESIGN_CONFIG=/absolute/path/to/Xgpu/physical_design/config.mk
```

## Budget (₹10,000) & Educational Fabrication Reality

- **Open-source EDA toolchain:** ₹0 (Yosys, OpenROAD, OpenLane 2, Icarus Verilog, `sv2v`, Cocotb, KLayout, Magic, Netgen, and SKY130 PDK are free and open-source).
- **Shuttle / MPW cost:** According to the [Tiny Tapeout Pricing Calculator](https://app.tinytapeout.com/calculator), a single SKY130 digital tile is **€70** (~₹6,500 tile-only; €385 / ~₹35,000+ with DevKit PCB and shipping). However, 1 Tiny Tapeout tile provides only 24 user I/O pins and ~1,000 gates, whereas XGPU currently uses **183 I/O pins** plus 8 combinational dividers and 1,024 register-file flip-flops.
- **Practical path within ₹10,000:** Keep all RTL-to-GDSII verification on free open-source tools + GitHub Actions, then apply for academic/educational MPW grants or Tiny Tapeout scholarships (after adding a serial/multiplexed I/O wrapper or Caravel/Wishbone bus bridge if targeting a pin-constrained shuttle).

## Blockers Before Fabrication

1. **Unexecuted physical verification:** Full OpenLane 2 / OpenROAD synthesis, STA (setup/hold across PVT corners), Magic/KLayout DRC, Netgen LVS, and antenna checks must run and pass cleanly on the target PDK.
2. **Pin count & padframe:** 183 raw signal pins require either a full padframe + package selection or a bus bridge (e.g., Wishbone/AXI-Lite/SPI) to fit educational shuttle I/O limits (such as Caravel's 38 GPIOs or Tiny Tapeout's 24 IOs).
3. **Combinational dividers (`alu.sv`):** 8 instances of single-cycle `rs / rt` will dominate area and critical path; pipelining or sharing a multi-cycle divider is recommended before timing-critical tapeout, and divide-by-zero behavior should be explicitly defined.
4. **Reset & synthesis hygiene:** `controller.sv` assigns scalar `0` to unpacked arrays and mixes blocking/non-blocking assignments; `dcr.sv` has a trailing port comma; 10 modules use `input reg` port syntax.
5. **External GDDR5 architecture:** Requires a completely different process node, licensed/custom high-speed PHY IP, flip-chip BGA package, and PCB beyond SKY130 and a ₹10,000 budget.
