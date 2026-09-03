---
name: homeos-inventory
license: MIT OR Apache-2.0
description: >-
  Consult at the start of any task that involves shell work: searching or
  transforming files, writing scripts, running toolchains. homeos, the
  user-level machine manager, is a CLI that keeps an inventory of the tools,
  applications, and agent skills the user installed through it, and it often
  holds tools that make the job faster or possible (ripgrep, jq,
  shellcheck, gh, mise, wsl, ...).
---

# homeos-inventory

Run `homeos package list --json` once per session and keep the result in
mind when choosing how to do shell work. Names are homeos package names
(`ripgrep`, not `rg`).

Only rows with `"installed": true` are available here; such an entry means
the tool is almost certainly there, and you do not need to verify each one.
Check whether a command is actually available only when it is not found or
behaves unexpectedly. Absence from the inventory does not mean absence from
the machine: it lists only what the user installed through homeos, so a tool
it does not mention may still be present.

When a tool you want is not on the machine, judge whether the user would
keep benefiting from it beyond this task. If so, propose installing it
through homeos (the `homeos-manage` skill). If a workaround serves this task
and the tool would not matter later, take the workaround without comment.
Never install it yourself.
