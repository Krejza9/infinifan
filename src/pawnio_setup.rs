//! Installs PawnIO with the official installer from `drivers\PawnIO_setup.exe`
//! (https://github.com/namazso/PawnIO.Setup, GPL-2). Runs only when the user asks, with a UAC prompt.
//!
//! The installer sits in a user-writable folder, so before it runs it is:
//! 1. copied to a private temp folder and locked against writes and deletion,
//! 2. checked for a valid Authenticode signature (`WinVerifyTrust`),
//! 3. checked to be signed by the PawnIO author (certificate "namazso.eu").
//!
//! Newer official releases pass; a substituted file does not.

use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use windows::Win32::Foundation::{CloseHandle, HWND, WAIT_OBJECT_0};
use windows::Win32::Security::Cryptography::{CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW};
use windows::Win32::Security::WinTrust::{
    WINTRUST_ACTION_GENERIC_VERIFY_V2, WINTRUST_DATA, WINTRUST_DATA_0, WINTRUST_FILE_INFO, WTD_CHOICE_FILE,
    WTD_REVOKE_NONE, WTD_STATEACTION_CLOSE, WTD_STATEACTION_VERIFY, WTD_UI_NONE, WTHelperGetProvSignerFromChain,
    WTHelperProvDataFromStateData, WinVerifyTrust,
};
use windows::Win32::System::Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RRF_SUBKEY_WOW6464KEY, RegGetValueW};
use windows::Win32::System::Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject};
use windows::Win32::UI::Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW};
use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
use windows::core::{HSTRING, PCWSTR, w};

/// Certificate name namazso signs PawnIO with.
const EXPECTED_SIGNER: &str = "namazso.eu";
const FILE_SHARE_READ: u32 = 0x1;

pub fn setup_path() -> PathBuf {
    crate::paths::drivers_dir().join("PawnIO_setup.exe")
}

/// Is PawnIO installed (same check as LibreHardwareMonitor)?
pub fn is_installed() -> bool {
    let mut size = 0u32;
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\PawnIO"),
            w!("DisplayVersion"),
            RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
            None,
            None,
            Some(&mut size),
        )
        .is_ok()
    }
}

/// Installs (`true`) or uninstalls (`false`) PawnIO. Blocks until the installer exits.
pub fn run(install: bool) -> Result<(), String> {
    let source = setup_path();
    if !source.exists() {
        return Err(tr!("setup.not_found", path = source.display()));
    }
    let dir = std::env::temp_dir().join(format!("InfiniFan-{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join("PawnIO_setup.exe");
    let result = (|| {
        std::fs::copy(&source, &path).map_err(|e| tr!("setup.copy", e = e))?;
        // Lock: while we hold the handle, nobody else can overwrite or delete the file.
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(FILE_SHARE_READ)
            .open(&path)
            .map_err(|e| tr!("setup.lock", e = e))?;
        verify(&path)?;
        let r = execute(&path, if install { "-install -silent" } else { "-uninstall -silent" });
        drop(lock);
        r
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

/// Checks the signature and the signer; returns the certificate name.
pub fn verify(path: &Path) -> Result<String, String> {
    let wpath = HSTRING::from(path.as_os_str());
    let mut file = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: PCWSTR(wpath.as_ptr()),
        ..Default::default()
    };
    let mut data = WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        fdwRevocationChecks: WTD_REVOKE_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut file },
        dwStateAction: WTD_STATEACTION_VERIFY,
        ..Default::default()
    };
    let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
    unsafe {
        let r = WinVerifyTrust(HWND::default(), &mut action, (&mut data as *mut WINTRUST_DATA).cast());
        let signer = if r == 0 { signer_name(&data) } else { None };
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(HWND::default(), &mut action, (&mut data as *mut WINTRUST_DATA).cast());
        if r != 0 {
            return Err(tr!("setup.bad_sig", code = format!("0x{:08X}", r as u32)));
        }
        match signer {
            Some(name) if name == EXPECTED_SIGNER => Ok(name),
            Some(name) => Err(tr!("setup.wrong_signer", name = name)),
            None => Err(tr!("setup.no_signer")),
        }
    }
}

/// Certificate name of the first (primary) signer.
unsafe fn signer_name(data: &WINTRUST_DATA) -> Option<String> {
    unsafe {
        let prov = WTHelperProvDataFromStateData(data.hWVTStateData);
        if prov.is_null() {
            return None;
        }
        let sgnr = WTHelperGetProvSignerFromChain(prov, 0, false, 0);
        if sgnr.is_null() || (*sgnr).csCertChain == 0 || (*sgnr).pasCertChain.is_null() {
            return None;
        }
        let cert = (*(*sgnr).pasCertChain).pCert;
        let mut buf = [0u16; 256];
        let n = CertGetNameStringW(cert, CERT_NAME_SIMPLE_DISPLAY_TYPE, 0, None, Some(&mut buf));
        (n > 1).then(|| String::from_utf16_lossy(&buf[..n as usize - 1]))
    }
}

fn execute(path: &Path, args: &str) -> Result<(), String> {
    let file = HSTRING::from(path.as_os_str());
    let params = HSTRING::from(args);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    unsafe {
        ShellExecuteExW(&mut info).map_err(|e| {
            if e.code().0 as u32 == 0x800704C7 {
                tr!("setup.uac_cancelled")
            } else {
                tr!("setup.start", e = e)
            }
        })?;
        if info.hProcess.is_invalid() {
            return Err(tr!("setup.not_started"));
        }
        let wait = WaitForSingleObject(info.hProcess, INFINITE);
        let mut code = 1u32;
        let _ = GetExitCodeProcess(info.hProcess, &mut code);
        let _ = CloseHandle(info.hProcess);
        if wait != WAIT_OBJECT_0 || code != 0 {
            return Err(tr!("setup.exit_code", code = code));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORIGINAL: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/pawnio/PawnIO_setup.exe");

    /// The official installer passes, a corrupted one does not (nothing is run).
    #[test]
    fn verifies_signer() {
        let dir = std::env::temp_dir().join("infinifan-test-sig");
        std::fs::create_dir_all(&dir).unwrap();
        let good = std::fs::read(ORIGINAL).unwrap();
        let p = dir.join("PawnIO_setup.exe");
        std::fs::write(&p, &good).unwrap();
        assert_eq!(verify(&p).unwrap(), EXPECTED_SIGNER);

        let mut bad = good.clone();
        bad[2000] ^= 0xFF;
        let q = dir.join("bad.exe");
        std::fs::write(&q, bad).unwrap();
        assert!(verify(&q).is_err());

        // A file signed by someone else (Microsoft) must be rejected.
        let other = PathBuf::from(std::env::var("SystemRoot").unwrap()).join("System32").join("notepad.exe");
        if other.exists() {
            assert!(verify(&other).is_err());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
