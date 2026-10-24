//! HID protocol of the Lian Li SL Infinity controller (ENE 6K77, USB 0CF2:A102, interface 1).
//! Byte for byte the same as L-Connect 3.

use std::thread::sleep;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use hidapi::{HidApi, HidDevice};

pub const VID: u16 = 0x0CF2;
pub const PID: u16 = 0xA102;
const REPORT_ID: u8 = 0xE0;
/// L-Connect 3 waits 20 ms after every write.
const WRITE_DELAY: Duration = Duration::from_millis(20);

pub const GROUP_COUNT: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub struct SlInfinity {
    dev: HidDevice,
}

impl SlInfinity {
    pub fn open(api: &HidApi) -> Result<Self> {
        let info = api
            .device_list()
            .find(|d| d.vendor_id() == VID && d.product_id() == PID && d.interface_number() == 1)
            .or_else(|| api.device_list().find(|d| d.vendor_id() == VID && d.product_id() == PID))
            .context(crate::tr!("dev.not_found"))?;
        let dev = api
            .open_path(info.path())
            .context(crate::tr!("dev.open_failed"))?;
        Ok(Self { dev })
    }

    fn feature(&self, data: &[u8]) -> Result<()> {
        let r = self.dev.send_feature_report(data);
        sleep(WRITE_DELAY);
        r.context("send_feature_report")
    }

    fn output(&self, data: &[u8]) -> Result<()> {
        let r = self.dev.write(data);
        sleep(WRITE_DELAY);
        r.map(|_| ()).context("write")
    }

    fn input(&self, len: usize) -> Result<Vec<u8>> {
        let mut buf = [0u8; 65];
        buf[0] = REPORT_ID;
        let n = self.dev.get_input_report(&mut buf).context("get_input_report")?;
        if n < len {
            bail!("short reply ({n} B)");
        }
        Ok(buf[1..=len].to_vec())
    }

    /// Firmware version (5 bytes; L-Connect 3 only shows them as raw data).
    pub fn firmware_version(&self) -> Result<Vec<u8>> {
        self.feature(&[REPORT_ID, 0x50, 0x01])?;
        self.input(5)
    }

    /// RPM of the 4 groups.
    pub fn fan_speeds(&self) -> Result<[u16; GROUP_COUNT]> {
        self.feature(&[REPORT_ID, 0x50, 0x00])?;
        let d = self.input(8)?;
        Ok(std::array::from_fn(|i| u16::from_be_bytes([d[i * 2], d[i * 2 + 1]])))
    }

    /// PWM duty in % (L-Connect 3 sends 1 = stop, otherwise 10–100).
    pub fn set_fan_duty(&self, group: usize, duty: u8) -> Result<()> {
        self.feature(&[REPORT_ID, 0x20 | (group as u8 & 0x0F), 0x00, duty])
    }

    pub fn set_effect(&self, port: u8, mode: u8, speed: u8, direction: u8, brightness: u8) -> Result<()> {
        self.feature(&[REPORT_ID, 0x10 | (port & 0x0F), mode, speed, direction, brightness])
    }

    /// Colours for a port; the device expects R, B, G order.
    pub fn set_colors(&self, port: u8, colors: &[Rgb]) -> Result<()> {
        if colors.is_empty() {
            return Ok(());
        }
        let mut data = Vec::with_capacity(2 + colors.len() * 3);
        data.push(REPORT_ID);
        data.push(0x30 | (port & 0x0F));
        for c in colors {
            data.extend_from_slice(&[c.0, c.2, c.1]);
        }
        self.output(&data)
    }

    pub fn ping(&self, group: usize, speed: u8, brightness: u8) -> Result<()> {
        self.feature(&[REPORT_ID, 0x10 | ((group as u8 * 2) & 0x0F), 62, speed, 0, brightness])
    }

    pub fn set_frame(&self, frame: u16) -> Result<()> {
        let [hi, lo] = frame.to_be_bytes();
        self.feature(&[REPORT_ID, 0x60, hi, lo])
    }

    /// Group speed is driven by motherboard PWM (true) or by this program (false).
    pub fn set_fan_mb_sync(&self, group: usize, sync: bool) -> Result<()> {
        let v = (1u8 << (group + 4)) | ((sync as u8) << group);
        self.feature(&[REPORT_ID, 0x10, 0x62, v, 0, 0])
    }

    /// RGB is driven by the motherboard ARGB header.
    pub fn set_lighting_mb_sync(&self, sync: bool) -> Result<()> {
        self.feature(&[REPORT_ID, 0x10, 0x61, sync as u8, 0, 0])
    }

    pub fn set_merge_order(&self, order: [u8; GROUP_COUNT]) -> Result<()> {
        self.feature(&[REPORT_ID, 0x10, 0x63, order[0], order[1], order[2], order[3], 0x08])
    }

    pub fn set_quantity(&self, group: usize, quantity: u8) -> Result<()> {
        self.feature(&[REPORT_ID, 0x10, 0x60, group as u8 + 1, quantity, 0])
    }
}
