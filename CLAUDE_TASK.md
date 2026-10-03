# CLAUDE_TASK

## Goal
Review the next xdos kernel hardening task.

## Current roadmap
1. Preemptive user scheduling
2. SMP/APIC
3. Copy-on-write fork
4. Real NIC (virtio)

## Task
Study the existing xdos code and determine the safest first implementation step.

Focus on:
- Rust kernel architecture
- scheduler/process code
- interrupt/context switching
- x86_64 assembly

Do not implement changes yet.

Return:
- files that need review
- current limitations
- proposed implementation plan
- risks
- exact next coding task for Cursor

Follow workflowMD.md and AI_TODO.md.
