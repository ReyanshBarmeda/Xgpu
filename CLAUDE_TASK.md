# CLAUDE TASK

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

## Commit Rule
Before making or committing any changes:
- Never leave incomplete, broken, or half-implemented code just because the commit point is near.
- Only commit when the current task is complete and tested.
- If the task cannot be safely completed, stop and explain what remains.
- Do not create a commit just to save unfinished work.

## Output
Return:
- files that need review
- current limitations
- proposed implementation plan
- risks
- exact next coding task for Cursor

Follow workflowMD.md and AI_TODO.md.
