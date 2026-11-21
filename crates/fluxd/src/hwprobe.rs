//! Hardware probe from /proc and /sys. `<state>/hardware.json` overrides it,
//! which is how demos and VMs simulate, for example, an NVIDIA GPU.

use std::fs;
use std::path::Path;

use flux_core::hardware::{CpuInfo, DiskInfo, GpuInfo, GpuVendor, HardwareInfo, NetInterface};

fn read(p: impl AsRef<Path>) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_string())
}

fn link_name(p: impl AsRef<Path>) -> Option<String> {
    fs::read_link(p).ok().and_then(|t| t.file_name().map(|n| n.to_string_lossy().into_owned()))
}

pub fn probe(state_dir: &Path) -> HardwareInfo {
    let override_path = state_dir.join("hardware.json");
    if let Some(src) = read(&override_path) {
        match serde_json::from_str::<HardwareInfo>(&src) {
            Ok(mut hw) => {
                hw.source = "override".into();
                return hw;
            }
            Err(e) => tracing::warn!("ignoring invalid {}: {e}", override_path.display()),
        }
    }

    let mut hw = HardwareInfo { source: "probe".into(), ..Default::default() };

    if let Some(cpuinfo) = read("/proc/cpuinfo") {
        let field = |name: &str| {
            cpuinfo.lines().find_map(|l| {
                let (k, v) = l.split_once(':')?;
                (k.trim() == name).then(|| v.trim().to_string())
            })
        };
        hw.cpu = CpuInfo {
            vendor: field("vendor_id").unwrap_or_default(),
            model: field("model name").unwrap_or_default(),
            threads: cpuinfo.lines().filter(|l| l.starts_with("processor")).count() as u32,
            freq_driver: read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_driver"),
        };
        if cpuinfo.lines().any(|l| l.starts_with("flags") && l.contains(" hypervisor")) {
            hw.virtualization = Some("hypervisor".into());
        }
    }
    if let Some(meminfo) = read("/proc/meminfo") {
        hw.memory_mb = meminfo
            .lines()
            .find_map(|l| l.strip_prefix("MemTotal:"))
            .and_then(|v| v.split_whitespace().next()?.parse::<u64>().ok())
            .map(|kb| kb / 1024)
            .unwrap_or(0);
    }
    if let Ok(rd) = fs::read_dir("/sys/bus/pci/devices") {
        for dev in rd.flatten() {
            let p = dev.path();
            let class = read(p.join("class")).unwrap_or_default();
            if !class.starts_with("0x03") {
                continue;
            }
            let vendor_id = read(p.join("vendor")).unwrap_or_default();
            hw.gpus.push(GpuInfo {
                vendor: GpuVendor::from_pci_id(&vendor_id),
                vendor_id,
                device_id: read(p.join("device")).unwrap_or_default(),
                driver: link_name(p.join("driver")),
            });
        }
    }
    hw.uefi = Path::new("/sys/firmware/efi").exists();
    if hw.uefi
        && let Ok(rd) = fs::read_dir("/sys/firmware/efi/efivars")
    {
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with("SecureBoot-")
                && let Ok(bytes) = fs::read(e.path())
            {
                hw.secure_boot = bytes.get(4).map(|b| *b == 1);
            }
        }
    }
    if let Some(vendor) = read("/sys/class/dmi/id/sys_vendor") {
        let product = read("/sys/class/dmi/id/product_name").unwrap_or_default();
        let lower = format!("{vendor} {product}").to_lowercase();
        for (needle, name) in [
            ("qemu", "qemu"),
            ("kvm", "kvm"),
            ("vmware", "vmware"),
            ("virtualbox", "virtualbox"),
            ("amazon", "amazon-ec2"),
            ("google", "gce"),
            ("microsoft", "hyper-v"),
        ] {
            if lower.contains(needle) {
                hw.virtualization = Some(name.into());
            }
        }
    }
    hw.kernel_release = read("/proc/sys/kernel/osrelease").unwrap_or_default();
    hw.sched_ext_supported = Path::new("/sys/kernel/sched_ext").exists();
    hw.is_nixos = Path::new("/etc/NIXOS").exists();
    if let Ok(rd) = fs::read_dir("/sys/block") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with("loop")
                || name.starts_with("ram")
                || name.starts_with("zram")
                || name.starts_with("dm-")
            {
                continue;
            }
            let q = e.path().join("queue");
            let scheduler = read(q.join("scheduler")).and_then(|s| {
                s.split_whitespace().find(|w| w.starts_with('[')).map(|w| w.trim_matches(['[', ']']).to_string())
            });
            hw.disks.push(DiskInfo { name, rotational: read(q.join("rotational")).as_deref() == Some("1"), scheduler });
        }
    }
    if let Ok(rd) = fs::read_dir("/sys/class/net") {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == "lo" {
                continue;
            }
            let p = e.path();
            hw.network.push(NetInterface {
                wireless: p.join("wireless").exists(),
                driver: link_name(p.join("device/driver")),
                name,
            });
        }
    }
    hw
}
