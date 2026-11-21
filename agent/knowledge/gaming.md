# Gaming

## Profile and when to use it
Use `profile.apply` with `gaming` when someone wants the machine to "prioritize gaming", become a "gaming rig", or play Steam/Proton titles well. It switches to the Zen low-latency kernel, loads the `scx_lavd` sched_ext scheduler, sets the performance CPU governor, enables GameMode and Gamescope, installs Steam, Lutris, Heroic, ProtonUp-Qt, Wine and MangoHud, and applies the Neon Arcade theme. The kernel switch is tier 3 and takes effect after a reboot; the previous kernel stays in the boot menu.

## Latency without a kernel switch
For "lower latency" or "more FPS" without a reboot, prefer tier 2 runtime changes: `kernel.set_scheduler` with `lavd` (built for gaming and interactive loads, used on handheld PCs) and `kernel.tune` with `cpu_governor: performance`. `vm.max_map_count = 2147483642` helps some modern titles; `kernel.split_lock_mitigate = 0` avoids split-lock slowdowns in a few older games.

## GPUs and drivers
AMD and Intel GPUs work with `drivers.gpu = auto` (Mesa). NVIDIA GPUs need `drivers.configure` with `nvidia-open` for Turing (RTX 20) and newer, or `nvidia` for older cards. Only configure NVIDIA drivers when the hardware report shows an NVIDIA GPU; with Secure Boot on, the modules must be signed.

## Controllers and streaming
`drivers.configure` with `gaming_controllers: true` enables Xbox (xpadneo) and Steam controller support. Discord and OBS Studio install as user-level Flathub apps with `apps.install` (tier 1). Bluetooth controllers also need the `bluetooth` service.

## Look and feel for gaming
Gamers usually want bold, high-contrast dark themes: generate a palette from a neon accent such as #00e5ff (cyan), #ff2e97 (pink) or #39ff14 (green) with `design.generate_palette`, display fonts like Orbitron with Rubik for UI text, a grid or aurora wallpaper, and snappy motion.
