# Xgpu

A configurable, GPU-inspired hardware accelerator implemented in SystemVerilog and validated with cocotb-based simulation tests.

Xgpu is a small RTL project designed to explore how a programmable graphics-style compute pipeline can be structured: multiple compute cores, per-thread register files, scheduling logic, memory controllers, and a custom instruction set. The design is intentionally simple, modular, and easy to inspect, making it useful for learning and experimentation in hardware architecture.

## Why this project exists

This project aims to model the core ideas behind a GPU-like execution model:

- multiple cores executing work in parallel
- thread blocks with per-thread state
- instruction fetch and decode stages
- ALU and memory operations per thread
- off-chip memory access through controllers and channels
- a host-driven kernel launch flow

The code is organized to be readable and educational rather than production-optimized.

## Highlights

- Configurable GPU parameters for core count, threads per block, and memory widths
- Program memory and data memory interfaces
- Fetcher, decoder, scheduler, and dispatcher modules
- Per-thread register file and program counter logic
- Load/store unit (LSU) and arithmetic logic unit (ALU)
- Memory arbitration layer for multiple consumers
- Cocotb testbench suite for functional validation

## Architecture overview

The top-level design is centered around `src/gpu.sv`, which instantiates:

- a device control register (`dcr.sv`)
- a data-memory controller (`controller.sv`)
- a program-memory controller (`controller.sv`)
- a dispatch unit (`dispatch.sv`)
- multiple compute cores (`core.sv`)

Each core contains:

- instruction fetch logic (`fetcher.sv`)
- decode logic (`decoder.sv`)
- scheduling and control flow (`scheduler.sv`)
- per-thread ALU, LSU, PC, and register logic

This structure is designed to reflect a real GPU-style pipeline while staying small enough for simulation and study.

## Repository layout

```text
Xgpu/
├── Makefile                 # Build and simulation commands
├── .gitignore
├── docs/
│   └── images/              # Supporting images and docs assets
├── gds/                     # Generated layout / GDS-related artifacts
├── src/
│   ├── alu.sv               # Arithmetic logic unit
│   ├── controller.sv        # Memory arbitration controller
│   ├── core.sv              # Compute core pipeline
│   ├── dcr.sv               # Device control register
│   ├── decoder.sv           # Instruction decode
│   ├── dispatch.sv          # Thread / block dispatch
│   ├── fetcher.sv           # Instruction fetch
│   ├── gpu.sv               # Top-level GPU module
│   ├── lsu.sv               # Load/store unit
│   ├── pc.sv                # Program counter logic
│   ├── registers.sv         # Register file
│   └── scheduler.sv         # Scheduling and control flow
├── test/
│   ├── helpers/
│   ├── logs/
│   ├── test_matadd.py       # Addition kernel validation
│   └── test_matmul.py       # Matrix-multiply kernel validation
└── README.md
```

## Supported workflow

The repo is built around a simulation test flow rather than a full software runtime.

### Build and test

Prerequisites:

- `iverilog`
- `sv2v`
- `cocotb`
- Python environment with `cocotb` configured correctly

Run:

```bash
make compile
make test_matmul
```

Or the generic pattern:

```bash
make test_matmul
make test_matadd
```

The `Makefile` compiles the RTL, emits a flattened Verilog output, and runs the simulation under cocotb.

## Example behavior

The test suite includes a kernel that performs matrix multiplication in the custom instruction set. The test:

1. loads a program into program memory
2. loads matrix data into data memory
3. configures thread count
4. starts execution
5. watches the `done` signal
6. validates the resulting memory against expected values

This gives a concrete example of how the architecture executes real work.

## Learning value

This repository is useful for:

- understanding GPU-style pipeline organization
- studying how hardware threads and blocks are scheduled
- experimenting with custom ISA decoding and control flow
- building small RTL projects with verification in cocotb
- exploring memory controller design in hardware

## Current status

This is a research and learning-focused RTL project. It is not a general-purpose production GPU, but it is a solid implementation for understanding accelerator architecture and low-level hardware design patterns.

## Future ideas

Possible directions for extending the project include:

- richer instruction set support
- shared-memory and coherence models
- branch prediction or hazard detection
- more advanced scheduling policies
- VCD waveform analysis and debugging tools
- integration with a higher-level compiler or assembler

## License

This repository does not currently declare a license in the root files. If you plan to publish or share the project publicly, consider adding a license such as MIT or Apache-2.0.

## Credits

Built as a custom hardware accelerator exploration project in SystemVerilog with cocotb simulation.
