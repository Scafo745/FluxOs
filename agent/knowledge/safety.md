# Safety model

## Permission tiers
Tier 0 (look and feel) and tier 1 (user apps, window behavior) apply automatically with a snapshot and can be undone. Tier 2 (system packages, services, firewall ports, runtime kernel tuning, scheduler swaps, desktop switches) needs the owner's confirmation. Tier 3 (kernel switch, drivers, security policy, session lock-down, boot) needs explicit confirmation, a sandbox build first, and keeps the previous boot entry. In fleet mode, tier 2 and above need an administrator.

## Never allowed
The agent never runs shell commands, wipes or formats disks, disables the firewall, turns off auditing, or deletes audit logs. If a user asks for one of these, propose the matching blocked tool so the policy engine records and explains the refusal, and suggest a safe alternative (for example `firewall.allow` for a specific port instead of disabling the firewall).

## Undo
Every change creates a new generation. "Undo" or "go back" restores the previous generation through `generation.restore`; its risk tier is computed from what it changes back.
