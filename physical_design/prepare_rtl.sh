#!/usr/bin/env bash
# Prepare XGPU SystemVerilog RTL for Yosys / OpenROAD / OpenLane 2.
# Converts all 12 SystemVerilog modules in src/ into a single Verilog-2005 file
# using sv2v, avoiding the root Makefile's `-I src/*` argument bug.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
SRC_DIR="${REPO_ROOT}/src"
BUILD_DIR="${SCRIPT_DIR}/build"
OUT_FILE="${BUILD_DIR}/gpu_sv2v.v"

RTL_FILES=(
  "${SRC_DIR}/gpu.sv"
  "${SRC_DIR}/core.sv"
  "${SRC_DIR}/alu.sv"
  "${SRC_DIR}/controller.sv"
  "${SRC_DIR}/dcr.sv"
  "${SRC_DIR}/decoder.sv"
  "${SRC_DIR}/dispatch.sv"
  "${SRC_DIR}/fetcher.sv"
  "${SRC_DIR}/lsu.sv"
  "${SRC_DIR}/pc.sv"
  "${SRC_DIR}/registers.sv"
  "${SRC_DIR}/scheduler.sv"
)

if ! command -v sv2v >/dev/null 2>&1; then
  echo "ERROR: sv2v not found in PATH. Install sv2v before running RTL preparation." >&2
  exit 1
fi

for f in "${RTL_FILES[@]}"; do
  if [[ ! -f "${f}" ]]; then
    echo "ERROR: Required RTL source file not found: ${f}" >&2
    exit 1
  fi
done

mkdir -p "${BUILD_DIR}"

echo "[prepare_rtl] Converting ${#RTL_FILES[@]} SystemVerilog files with sv2v..."
sv2v "${RTL_FILES[@]}" -w "${OUT_FILE}"

if [[ ! -s "${OUT_FILE}" ]] || ! grep -q "^module gpu" "${OUT_FILE}"; then
  echo "ERROR: sv2v output validation failed (${OUT_FILE} missing or no 'module gpu')." >&2
  exit 1
fi

echo "[prepare_rtl] Generated ${OUT_FILE} ($(wc -l < "${OUT_FILE}") lines)"
