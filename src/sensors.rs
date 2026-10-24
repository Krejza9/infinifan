//! CPU and GPU temperatures.
//!
//! * CPU (AMD Zen): Tctl from SMN register 0x00059800 via the PawnIO driver, if it is installed
//!   (PawnIO 2.x is a Plug-and-Play driver – without an INF install it creates no device,
//!   so it can't just be loaded temporarily as a service). The `AMDFamily17.bin` module comes from PawnIO.Modules (LGPL-2.1).
//! * GPU: temperature from the Windows graphics kernel (D3DKMT, same as Task Manager) – no extra
//!   library (the earlier NVML approach used ~20 MB of memory).

use windows::Win32::Foundation::{CloseHandle, GENERIC_READ, GENERIC_WRITE, HANDLE, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::Win32::System::IO::DeviceIoControl;
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::w;

const AMD_FAMILY17_MODULE: &[u8] = include_bytes!("../assets/pawnio/AMDFamily17.bin");
const SMN_THM_TCON_CUR_TMP: u64 = 0x0005_9800;

// pawnio_um.h: CTL_CODE(41394, fn, METHOD_BUFFERED, FILE_ANY_ACCESS)
const fn ctl_code(function: u32) -> u32 {
    (41394 << 16) | (function << 2)
}
const IOCTL_PIO_LOAD_BINARY: u32 = ctl_code(0x821);
const IOCTL_PIO_EXECUTE_FN: u32 = ctl_code(0x841);
const FN_NAME_LENGTH: usize = 32;

/// Why the CPU temperature is unavailable (shown in the GUI).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum CpuSensorStatus {
    Ok,
    PawnIoMissing,
    NeedsAdmin,
    Unsupported(String),
}

struct PawnIo {
    device: HANDLE,
    pci_mutex: HANDLE,
}

// The handles are kernel objects, used only from the control thread.
unsafe impl Send for PawnIo {}

impl PawnIo {
    fn open() -> Result<Self, CpuSensorStatus> {
        unsafe {
            let device = CreateFileW(
                w!("\\\\?\\GLOBALROOT\\Device\\PawnIO"),
                (GENERIC_READ | GENERIC_WRITE).0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
            .map_err(|e| {
                if e.code() == windows::Win32::Foundation::E_ACCESSDENIED {
                    CpuSensorStatus::NeedsAdmin
                } else {
                    CpuSensorStatus::PawnIoMissing
                }
            })?;
            if let Err(e) = DeviceIoControl(
                device,
                IOCTL_PIO_LOAD_BINARY,
                Some(AMD_FAMILY17_MODULE.as_ptr().cast()),
                AMD_FAMILY17_MODULE.len() as u32,
                None,
                0,
                None,
                None,
            ) {
                let _ = CloseHandle(device);
                return Err(CpuSensorStatus::Unsupported(crate::tr!("sensor.not_zen", e = e)));
            }
            // The same mutex is used by other tools (LHM, HWiNFO…) to access PCI configuration space.
            let pci_mutex = CreateMutexW(None, false, w!("Global\\Access_PCI")).unwrap_or_default();
            Ok(Self { device, pci_mutex })
        }
    }

    fn execute(&self, name: &str, input: &[u64], output: &mut [u64]) -> Option<usize> {
        let mut buf = vec![0u8; FN_NAME_LENGTH + input.len() * 8];
        let n = name.len().min(FN_NAME_LENGTH - 1);
        buf[..n].copy_from_slice(&name.as_bytes()[..n]);
        for (i, v) in input.iter().enumerate() {
            buf[FN_NAME_LENGTH + i * 8..FN_NAME_LENGTH + i * 8 + 8].copy_from_slice(&v.to_le_bytes());
        }
        let mut written = 0u32;
        unsafe {
            DeviceIoControl(
                self.device,
                IOCTL_PIO_EXECUTE_FN,
                Some(buf.as_ptr().cast()),
                buf.len() as u32,
                Some(output.as_mut_ptr().cast()),
                (output.len() * 8) as u32,
                Some(&mut written),
                None,
            )
            .ok()?;
        }
        Some(written as usize / 8)
    }

    fn read_smn(&self, address: u64) -> Option<u32> {
        let mut out = [0u64; 1];
        unsafe {
            let locked = !self.pci_mutex.is_invalid() && {
                let r = WaitForSingleObject(self.pci_mutex, 100);
                r == WAIT_OBJECT_0 || r == WAIT_ABANDONED
            };
            let n = self.execute("ioctl_read_smn", &[address], &mut out);
            if locked {
                let _ = ReleaseMutex(self.pci_mutex);
            }
            (n? >= 1).then_some(out[0] as u32)
        }
    }

    /// Tctl in °C (AMD PPR: CUR_TEMP = bits 31:21 × 0.125; subtract 49 if CUR_TEMP_RANGE_SEL (bit 19) is set).
    fn read_tctl(&self) -> Option<f32> {
        let raw = self.read_smn(SMN_THM_TCON_CUR_TMP)?;
        let mut t = ((raw >> 21) & 0x7FF) as f32 * 0.125;
        if raw & (1 << 19) != 0 {
            t -= 49.0;
        }
        (-20.0..=130.0).contains(&t).then_some(t)
    }
}

impl Drop for PawnIo {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.device);
            if !self.pci_mutex.is_invalid() {
                let _ = CloseHandle(self.pci_mutex);
            }
        }
    }
}

pub struct Sensors {
    pawnio: Option<PawnIo>,
    pub cpu_status: CpuSensorStatus,
    gpu: Option<GpuSensor>,
    pub gpu_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Temperatures {
    pub cpu: Option<f32>,
    pub gpu: Option<f32>,
}

impl Temperatures {
    /// Temperature for the curve source; if the chosen source is missing, the other one as a fallback (bool = fallback).
    pub fn for_source(&self, source: crate::curve::Source) -> (Option<f32>, bool) {
        let (want, other) = match source {
            crate::curve::Source::Cpu => (self.cpu, self.gpu),
            crate::curve::Source::Gpu => (self.gpu, self.cpu),
        };
        match want {
            Some(t) => (Some(t), false),
            None => (other, other.is_some()),
        }
    }
}

impl Sensors {
    pub fn new() -> Self {
        let (pawnio, cpu_status) = match PawnIo::open() {
            Ok(p) => (Some(p), CpuSensorStatus::Ok),
            Err(e) => (None, e),
        };
        if pawnio.is_none() {
            log::warn!("CPU temperature unavailable: {cpu_status:?}");
        }
        let gpu = GpuSensor::open();
        if gpu.is_none() {
            log::warn!("GPU temperature unavailable (the graphics driver does not report it)");
        }
        let gpu_name = gpu.as_ref().map(|g| g.name.clone());
        Self { pawnio, cpu_status, gpu, gpu_name }
    }

    pub fn read(&self) -> Temperatures {
        let cpu = self.pawnio.as_ref().and_then(PawnIo::read_tctl);
        let gpu = self.gpu.as_ref().and_then(GpuSensor::temperature);
        Temperatures { cpu, gpu }
    }
}

pub fn is_elevated() -> bool {
    unsafe { windows::Win32::UI::Shell::IsUserAnAdmin().as_bool() }
}

/// GPU temperature via D3DKMT (gdi32): the Windows graphics kernel reports it for WDDM 2.4+ drivers.
struct GpuSensor {
    adapter: u32,
    name: String,
}

impl GpuSensor {
    fn query(adapter: u32) -> Option<f32> {
        use windows::Wdk::Graphics::Direct3D::{D3DKMT_ADAPTER_PERFDATA, D3DKMT_QUERYADAPTERINFO, D3DKMTQueryAdapterInfo, KMTQAITYPE_ADAPTERPERFDATA};
        let mut perf = D3DKMT_ADAPTER_PERFDATA::default();
        let mut q = D3DKMT_QUERYADAPTERINFO {
            hAdapter: adapter,
            Type: KMTQAITYPE_ADAPTERPERFDATA,
            pPrivateDriverData: (&mut perf as *mut D3DKMT_ADAPTER_PERFDATA).cast(),
            PrivateDriverDataSize: size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
        };
        let ok = unsafe { D3DKMTQueryAdapterInfo(&mut q) }.is_ok();
        // Temperature is in tenths of °C; 0 = the adapter doesn't report it (e.g. Microsoft Basic Render).
        (ok && perf.Temperature > 0).then(|| perf.Temperature as f32 / 10.0)
    }

    fn name(adapter: u32) -> String {
        use windows::Wdk::Graphics::Direct3D::{D3DKMT_ADAPTERREGISTRYINFO, D3DKMT_QUERYADAPTERINFO, D3DKMTQueryAdapterInfo, KMTQAITYPE_ADAPTERREGISTRYINFO};
        let mut info = D3DKMT_ADAPTERREGISTRYINFO::default();
        let mut q = D3DKMT_QUERYADAPTERINFO {
            hAdapter: adapter,
            Type: KMTQAITYPE_ADAPTERREGISTRYINFO,
            pPrivateDriverData: (&mut info as *mut D3DKMT_ADAPTERREGISTRYINFO).cast(),
            PrivateDriverDataSize: size_of::<D3DKMT_ADAPTERREGISTRYINFO>() as u32,
        };
        if unsafe { D3DKMTQueryAdapterInfo(&mut q) }.is_err() {
            return "GPU".into();
        }
        let n = info.AdapterString.iter().position(|c| *c == 0).unwrap_or(info.AdapterString.len());
        String::from_utf16_lossy(&info.AdapterString[..n])
    }

    /// Picks the adapter that reports a temperature (with multiple GPUs the hottest = the discrete card).
    fn open() -> Option<Self> {
        use windows::Wdk::Graphics::Direct3D::{D3DKMT_ADAPTERINFO, D3DKMT_CLOSEADAPTER, D3DKMT_ENUMADAPTERS2, D3DKMTCloseAdapter, D3DKMTEnumAdapters2};
        unsafe {
            let mut e = D3DKMT_ENUMADAPTERS2::default();
            if D3DKMTEnumAdapters2(&mut e).is_err() || e.NumAdapters == 0 {
                return None;
            }
            let mut list = vec![D3DKMT_ADAPTERINFO::default(); e.NumAdapters as usize];
            e.pAdapters = list.as_mut_ptr();
            if D3DKMTEnumAdapters2(&mut e).is_err() {
                return None;
            }
            list.truncate(e.NumAdapters as usize);
            let best = list
                .iter()
                .filter_map(|a| Self::query(a.hAdapter).map(|t| (a.hAdapter, t)))
                .max_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(h, _)| h);
            // Close the other adapters, keep the chosen one open for reading.
            for a in &list {
                if Some(a.hAdapter) != best {
                    let _ = D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER { hAdapter: a.hAdapter });
                }
            }
            best.map(|adapter| Self { adapter, name: Self::name(adapter) })
        }
    }

    fn temperature(&self) -> Option<f32> {
        Self::query(self.adapter)
    }
}

impl Drop for GpuSensor {
    fn drop(&mut self) {
        use windows::Wdk::Graphics::Direct3D::{D3DKMT_CLOSEADAPTER, D3DKMTCloseAdapter};
        unsafe {
            let _ = D3DKMTCloseAdapter(&D3DKMT_CLOSEADAPTER { hAdapter: self.adapter });
        }
    }
}

#[cfg(test)]
mod gpu_tests {
    /// The D3DKMT GPU temperature must match nvidia-smi (NVML) when available.
    #[test]
    fn d3dkmt_matches_nvidia_smi() {
        let Some(g) = super::GpuSensor::open() else { return };
        let t = g.temperature().unwrap();
        println!("D3DKMT: {} – {t} °C", g.name);
        let Ok(out) = std::process::Command::new("nvidia-smi").args(["--query-gpu=temperature.gpu", "--format=csv,noheader"]).output() else { return };
        let Ok(smi) = String::from_utf8_lossy(&out.stdout).trim().lines().next().unwrap_or("").parse::<f32>() else { return };
        println!("nvidia-smi: {smi} °C");
        assert!((t - smi).abs() <= 2.0, "D3DKMT {t} vs nvidia-smi {smi}");
    }
}
