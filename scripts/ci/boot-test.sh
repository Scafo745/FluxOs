#!/usr/bin/env bash
# Boots the FluxOS appliance from its OVA in QEMU, with hardware like the
# VirtualBox VM (BIOS, SATA disk, e1000 NIC), runs the in-image self-test
# (scripts/ci/selftest.sh) and saves the serial log and screenshots.
#
#   scripts/ci/boot-test.sh <directory with the .ova> [output directory]
#
# Needs qemu-system-x86_64, qemu-img, socat and tar; uses KVM when /dev/kvm
# is writable. Exits 0 when the self-test reports DONE ok.
#
# Graphics: VGA=std (default, bochs-drm) or VGA=vmware. VirtualBox's VMSVGA
# and VMware's SVGA use the vmwgfx driver; QEMU's old VMware SVGA model is
# only a rough stand-in for them.
set -euo pipefail

ova=$(find "$1" -maxdepth 1 -name '*.ova' | head -n 1)
[ -n "$ova" ] || { echo "no .ova in $1" >&2; exit 2; }
out=$(mkdir -p "${2:-boot-test}" && cd "${2:-boot-test}" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

echo "unpacking $(basename "$ova") ($(du -h "$ova" | cut -f1))"
tar -C "$work" -xf "$ova"
ls -l "$work"
vmdk=$(find "$work" -name '*.vmdk' | head -n 1)
qemu-img convert -O qcow2 "$vmdk" "$work/disk.qcow2"

mon="$work/monitor.sock"
serial="$out/serial.log"
: >"$serial"
if [ -w /dev/kvm ]; then accel=(-enable-kvm -cpu host); else
  echo "warning: /dev/kvm is not writable, falling back to slow emulation" >&2
  accel=(-cpu max)
fi

qemu-system-x86_64 "${accel[@]}" -smp 2 -m 4096 \
  -drive "file=$work/disk.qcow2,if=none,id=disk0,format=qcow2" \
  -device ahci,id=sata -device ide-hd,drive=disk0,bus=sata.0 \
  -netdev user,id=net0 -device e1000,netdev=net0 \
  -vga "${VGA:-std}" -display none \
  -smbios type=11,value=io.systemd.credential:fluxos.selftest=1 \
  -serial "file:$serial" \
  -monitor "unix:$mon,server,nowait" &
qemu=$!

hmp() { echo "$1" | socat - "UNIX-CONNECT:$mon" >/dev/null 2>&1 || true; }
shot() {
  hmp "screendump $out/$1.png -f png"
  echo "[$(( $(date +%s) - start ))s] screenshot $1"
}

start=$(date +%s)
deadline=$(( start + ${TIMEOUT:-2400} ))
taken=" "
status=timeout
tick=0
while kill -0 "$qemu" 2>/dev/null && [ "$(date +%s)" -lt "$deadline" ]; do
  sleep 3
  tick=$((tick + 1))
  # The boot sequence: boot menu, splash, the shell's own boot screen.
  case $tick in 2 | 4 | 6 | 9 | 12 | 16 | 20) shot "$(printf 'boot-%03ds' $(( $(date +%s) - start )))" ;; esac
  for m in $(grep -ao 'FLUXOS-SELFTEST screenshot [a-z-]*' "$serial" | awk '{print $3}'); do
    case "$taken" in *" $m "*) ;; *)
      shot "$m"
      taken="$taken$m "
      ;;
    esac
  done
  if grep -aq 'FLUXOS-SELFTEST DONE' "$serial"; then
    if grep -aq 'FLUXOS-SELFTEST DONE ok' "$serial"; then status=ok; else status=fail; fi
    break
  fi
done
shot last
hmp quit
wait "$qemu" 2>/dev/null || true

echo "--- self-test report"
grep -a 'FLUXOS-SELFTEST\|  | ' "$serial" || true
echo "--- self-test: $status after $(( $(date +%s) - start ))s"
[ "$status" = ok ]
