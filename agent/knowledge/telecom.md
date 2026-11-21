# Telecom and network operations

## Profile and when to use it
Use `profile.apply` with `telecom` for a network operations center (NOC), ISP or carrier operations desk, or a network engineer's workstation. It switches to Hyprland tiling with the ops-center style (eight workspaces, dense IBM Plex type, minimal motion), installs Wireshark, termshark, tcpdump, Nmap, mtr, iperf3, Net-SNMP, a NETCONF client (Python ncclient), sngrep, SIPp, baresip, minicom, picocom, tmux, Ansible, ipcalc and dig, and enables chrony time sync and a five-minute auto-lock.

## Tuned networking
The profile raises socket buffers (`net.core.rmem_max`, `net.core.wmem_max`, `net.ipv4.tcp_rmem`, `net.ipv4.tcp_wmem` up to 128 MiB), increases `net.core.netdev_max_backlog`, and uses BBR congestion control with the fq qdisc. Routing (`net.ipv4.ip_forward`) is tier 3 and should only be enabled when the machine must forward packets.

## Packet capture and access
Wireshark is installed through its NixOS module so capture works for members of the `wireshark` group. Nmap must only be used on networks the operator is authorized to scan. Inbound SSH (`services.enable` `ssh`) opens TCP 22 and is key-only with root login disabled.

## Dashboards and layouts
Operations desks keep dashboards in a browser and terminals in tmux. The ops-center style tiles windows with small gaps; a dark palette with a phosphor-green (#2ee6a6) or amber (#ffb020) accent reads well on wall screens.
