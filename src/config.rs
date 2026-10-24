use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::curve::{FanGroupConfig, FanSize};
use crate::device::GROUP_COUNT;
use crate::effects::LightingConfig;

/// Who else may control the device besides this program.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalControl {
    /// Lighting follows the motherboard ARGB header (Aura/Mystic Light/RGB Fusion…).
    pub mb_argb_sync: bool,
    /// Fan speed follows the motherboard PWM (BIOS, FanControl, …).
    pub mb_rpm_sync: bool,
    /// Lighting is controlled by another program (SignalRGB, OpenRGB), like the SignalRGB switch
    /// in L-Connect 3: lighting is not written, this program keeps controlling the fans.
    pub rgb_by_other_software: bool,
    /// Release the device completely (no writes, HID handle closed).
    pub release_device: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub fans: [FanGroupConfig; GROUP_COUNT],
    /// Number of fans in each group (0–4).
    pub quantity: [u8; GROUP_COUNT],
    pub lighting: LightingConfig,
    pub external: ExternalControl,
    /// Unused since the window/background split; kept for config compatibility.
    pub close_to_tray: bool,
    /// The user dismissed the PawnIO install offer at startup.
    pub pawnio_prompt_dismissed: bool,
    /// The "My colors" palette in the color picker.
    pub saved_colors: Vec<[u8; 3]>,
    /// UI language code (`locales/<code>.yml`); `None` = English.
    pub language: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            fans: std::array::from_fn(|_| FanGroupConfig::new(FanSize::Mm140)),
            quantity: [3; GROUP_COUNT],
            lighting: LightingConfig::default(),
            external: ExternalControl::default(),
            close_to_tray: true,
            pawnio_prompt_dismissed: false,
            saved_colors: Vec::new(),
            language: None,
        }
    }
}

impl AppConfig {
    pub fn path() -> PathBuf {
        crate::paths::config_file()
    }

    pub fn load() -> Self {
        // Migrate the config from older locations (next to the exe, %APPDATA%).
        let path = Self::path();
        if !path.exists() {
            for old in crate::paths::legacy_config_files() {
                if old.exists() && old != path && std::fs::copy(&old, &path).is_ok() {
                    // Remove old files in the program folder; keep the ones in %APPDATA% as a backup.
                    if old.starts_with(crate::paths::exe_dir()) {
                        let _ = std::fs::remove_file(&old);
                    }
                    log::info!("Config migrated from {}", old.display());
                    break;
                }
            }
        }
        if !Self::path().exists()
            && let Some(mut imported) = crate::lconnect_import::import()
        {
            imported.fans.iter_mut().for_each(FanGroupConfig::normalize);
            imported.save();
            return imported;
        }
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|s| serde_json::from_str::<Self>(&s).map_err(|e| log::warn!("Invalid config: {e}")).ok())
            .map(|mut c| {
                c.fans.iter_mut().for_each(FanGroupConfig::normalize);
                c
            })
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match serde_json::to_string_pretty(self) {
            Ok(s) => {
                // Write through a temp file so a crash cannot corrupt the config.
                let tmp = path.with_extension("json.tmp");
                if std::fs::write(&tmp, s).is_ok() {
                    let _ = std::fs::rename(&tmp, &path);
                }
            }
            Err(e) => log::error!("Saving the config failed: {e}"),
        }
    }
}
