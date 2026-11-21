//! Hardware facts used by compatibility checks. Probing lives in `fluxd`;
//! this crate only reasons about the result.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuVendor {
    Nvidia,
    Amd,
    Intel,
    Virtual,
    Other,
}

impl GpuVendor {
    pub fn from_pci_id(id: &str) -> Self {
        match id.trim().trim_start_matches("0x").to_ascii_lowercase().as_str() {
            "10de" => Self::Nvidia,
            "1002" | "1022" => Self::Amd,
            "8086" => Self::Intel,
            "1af4" | "1234" | "15ad" | "80ee" | "1b36" => Self::Virtual,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub vendor: GpuVendor,
    pub vendor_id: String,
    pub device_id: String,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CpuInfo {
    pub vendor: String,
    pub model: String,
    pub threads: u32,
    /// Active cpufreq driver, e.g. `amd-pstate-epp`, `intel_pstate`.
    pub freq_driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskInfo {
    pub name: String,
    pub rotational: bool,
    pub scheduler: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetInterface {
    pub name: String,
    pub wireless: bool,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareInfo {
    /// `probe`, `override` (from hardware.json) or `unknown`.
    pub source: String,
    pub arch: String,
    pub cpu: CpuInfo,
    pub memory_mb: u64,
    pub gpus: Vec<GpuInfo>,
    pub uefi: bool,
    pub secure_boot: Option<bool>,
    pub virtualization: Option<String>,
    pub kernel_release: String,
    pub sched_ext_supported: bool,
    pub is_nixos: bool,
    pub disks: Vec<DiskInfo>,
    pub network: Vec<NetInterface>,
}

impl Default for HardwareInfo {
    fn default() -> Self {
        Self {
            source: "unknown".into(),
            arch: std::env::consts::ARCH.into(),
            cpu: CpuInfo::default(),
            memory_mb: 0,
            gpus: Vec::new(),
            uefi: false,
            secure_boot: None,
            virtualization: None,
            kernel_release: String::new(),
            sched_ext_supported: false,
            is_nixos: false,
            disks: Vec::new(),
            network: Vec::new(),
        }
    }
}

impl HardwareInfo {
    pub fn has_gpu(&self, vendor: GpuVendor) -> bool {
        self.gpus.iter().any(|g| g.vendor == vendor)
    }

    /// True when we actually know which GPUs exist.
    pub fn gpus_known(&self) -> bool {
        self.source != "unknown"
    }

    pub fn summary(&self) -> String {
        let gpus = if self.gpus.is_empty() {
            "no GPU".to_string()
        } else {
            self.gpus.iter().map(|g| format!("{:?}", g.vendor).to_lowercase()).collect::<Vec<_>>().join("+")
        };
        format!(
            "{} {}t, {} MiB RAM, {}, {}{}",
            if self.cpu.model.is_empty() { "unknown CPU" } else { &self.cpu.model },
            self.cpu.threads,
            self.memory_mb,
            gpus,
            if self.uefi { "UEFI" } else { "BIOS/unknown firmware" },
            self.virtualization.as_deref().map(|v| format!(", virtualized ({v})")).unwrap_or_default()
        )
    }
}
