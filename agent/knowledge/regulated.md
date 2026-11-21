# Finance and healthcare (preview profiles)

## Compliance first
The `finance` and `healthcare` profiles are previews. Their rules (securities-regulator and record-keeping requirements for finance, HIPAA and local law for US healthcare) must be defined by the customer's compliance team before production use. Always mention this when applying them.

## Trading desks
`finance` uses the Zen low-latency kernel, the performance governor, no I/O scheduler for NVMe, busy polling, chrony precise time sync, blocked USB storage and a five-minute auto-lock, with a calm dark theme and a classic panel. Trading hours are usually protected with a change-freeze window in the operator policy (`policy.json`), which defers tier 2+ changes.

## Clinical workstations
`healthcare` uses GNOME with large, AAA-contrast text (Atkinson Hyperlegible Next), a two-minute auto-lock, blocked USB storage, smart card and badge readers (`smartcard` service), Remmina for secure remote desktop, browser-based record systems, and a shared account wiped on every boot so no patient data stays on the device.
