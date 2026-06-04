//! Windows implementation of `SystemProbe`. cfg(windows) only - not compiled on
//! Linux/CI-ubuntu; verified by compile+clippy on the windows-latest runner.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::ptr::{null, null_mut};

use super::probe::{ElevationFacts, RawFacts, SystemProbe};

pub struct WindowsProbe;

impl SystemProbe for WindowsProbe {
    fn gather(&self) -> RawFacts {
        let (build_number, ubr) = build_facts();
        let (virtualization_firmware_enabled, vm_monitor_mode_extensions, hypervisor_present) =
            wmi_facts();
        let (free_bytes_available, cfa_protected_path) = disk_facts();

        RawFacts {
            build_number,
            ubr,
            arch: native_arch(),
            virt_feature_present: virt_feature_present(),
            vm_monitor_mode_extensions,
            virtualization_firmware_enabled,
            hypervisor_present,
            known_vmm_services: known_vmm_services(),
            free_bytes_available,
            elevation: elevation_facts(),
            wsl_blocked: wsl_blocked(),
            inbox_wsl_blocked: inbox_wsl_blocked(),
            store_disabled: store_disabled(),
            cfa_mode: cfa_mode(),
            cfa_protected_path,
        }
    }
}

fn build_facts() -> (Option<u32>, Option<u32>) {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    match hklm.open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion") {
        Ok(key) => {
            let build_number = key
                .get_value::<String, _>("CurrentBuildNumber")
                .ok()
                .and_then(|value| value.parse::<u32>().ok());
            let ubr = key.get_value::<u32, _>("UBR").ok();
            (build_number, ubr)
        }
        Err(_) => (None, None),
    }
}

fn native_arch() -> u16 {
    use windows_sys::Win32::System::SystemInformation::{GetNativeSystemInfo, SYSTEM_INFO};

    unsafe {
        let mut si = std::mem::zeroed::<SYSTEM_INFO>();
        GetNativeSystemInfo(&mut si);
        si.Anonymous.Anonymous.wProcessorArchitecture
    }
}

fn virt_feature_present() -> bool {
    use windows_sys::Win32::System::Threading::{
        IsProcessorFeaturePresent, PF_VIRT_FIRMWARE_ENABLED,
    };

    unsafe { IsProcessorFeaturePresent(PF_VIRT_FIRMWARE_ENABLED) != 0 }
}

fn wmi_facts() -> (Option<bool>, Option<bool>, Option<bool>) {
    std::thread::spawn(|| {
        use serde::Deserialize;
        use wmi::WMIConnection;

        // wmi 0.18: WMIConnection::new() takes no args and initializes COM
        // internally (create_locator_or_init). The connection is !Send, so it
        // is created and used entirely within this dedicated thread.
        let con = WMIConnection::new().ok()?;

        #[derive(Deserialize)]
        #[serde(rename = "Win32_Processor")]
        #[serde(rename_all = "PascalCase")]
        struct Proc {
            virtualization_firmware_enabled: Option<bool>,
            vm_monitor_mode_extensions: Option<bool>,
        }

        #[derive(Deserialize)]
        #[serde(rename = "Win32_ComputerSystem")]
        #[serde(rename_all = "PascalCase")]
        struct Cs {
            hypervisor_present: Option<bool>,
        }

        let procs: Vec<Proc> = con.query().ok()?;
        let cs: Vec<Cs> = con.query().ok()?;
        let proc = procs.into_iter().next()?;
        let cs = cs.into_iter().next()?;

        Some((
            proc.virtualization_firmware_enabled,
            proc.vm_monitor_mode_extensions,
            cs.hypervisor_present,
        ))
    })
    .join()
    .ok()
    .flatten()
    .unwrap_or((None, None, None))
}

fn known_vmm_services() -> Vec<String> {
    use windows_sys::Win32::System::Services::{
        CloseServiceHandle, OpenSCManagerW, OpenServiceW, SC_MANAGER_CONNECT, SERVICE_QUERY_STATUS,
    };

    unsafe {
        let scm = OpenSCManagerW(null(), null(), SC_MANAGER_CONNECT);
        if scm.is_null() {
            return Vec::new();
        }

        let mut services = Vec::new();
        for name in ["vmci", "VBoxDrv"] {
            let wide = wide_null(name);
            let handle = OpenServiceW(scm, wide.as_ptr(), SERVICE_QUERY_STATUS);
            if !handle.is_null() {
                services.push(name.to_string());
                CloseServiceHandle(handle);
            }
        }

        CloseServiceHandle(scm);
        services
    }
}

fn disk_facts() -> (Option<u64>, Option<String>) {
    use windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;

    let path = match std::env::var("LOCALAPPDATA").or_else(|_| std::env::var("USERPROFILE")) {
        Ok(path) => path,
        Err(_) => return (None, None),
    };
    let wide = wide_null(&path);
    let mut free_avail = 0u64;

    let free = unsafe {
        if GetDiskFreeSpaceExW(wide.as_ptr(), &mut free_avail, null_mut(), null_mut()) != 0 {
            Some(free_avail)
        } else {
            None
        }
    };

    (free, Some(path))
}

fn elevation_facts() -> ElevationFacts {
    ElevationFacts {
        is_member_filtered: is_member_filtered(),
        elevation_type: elevation_type(),
    }
}

fn is_member_filtered() -> bool {
    use windows_sys::Win32::Security::{
        AllocateAndInitializeSid, CheckTokenMembership, FreeSid, SECURITY_NT_AUTHORITY,
    };
    use windows_sys::Win32::System::SystemServices::{
        DOMAIN_ALIAS_RID_ADMINS, SECURITY_BUILTIN_DOMAIN_RID,
    };

    unsafe {
        let mut sid = null_mut();
        let authority = SECURITY_NT_AUTHORITY;
        if AllocateAndInitializeSid(
            &authority,
            2,
            SECURITY_BUILTIN_DOMAIN_RID as u32,
            DOMAIN_ALIAS_RID_ADMINS as u32,
            0,
            0,
            0,
            0,
            0,
            0,
            &mut sid,
        ) == 0
        {
            return false;
        }

        let mut is_member = 0;
        let ok = CheckTokenMembership(null_mut(), sid, &mut is_member) != 0;
        FreeSid(sid);
        ok && is_member != 0
    }
}

fn elevation_type() -> u32 {
    use std::ffi::c_void;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{
        GetTokenInformation, TOKEN_ELEVATION_TYPE, TOKEN_QUERY, TokenElevationType,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token: HANDLE = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return 1;
        }

        let mut ty: TOKEN_ELEVATION_TYPE = 1;
        let mut ret_len = 0;
        let ok = GetTokenInformation(
            token,
            TokenElevationType,
            &mut ty as *mut _ as *mut c_void,
            std::mem::size_of::<TOKEN_ELEVATION_TYPE>() as u32,
            &mut ret_len,
        ) != 0;
        CloseHandle(token);

        if ok { ty as u32 } else { 1 }
    }
}

fn wsl_blocked() -> bool {
    wsl_policy_value("AllowWSL") == Some(0)
}

fn inbox_wsl_blocked() -> bool {
    wsl_policy_value("AllowInboxWSL") == Some(0)
}

fn wsl_policy_value(name: &str) -> Option<u32> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(r"Software\Policies\WSL")
        .ok()
        .and_then(|key| key.get_value::<u32, _>(name).ok())
}

fn store_disabled() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    hklm.open_subkey(r"SOFTWARE\Policies\Microsoft\WindowsStore")
        .ok()
        .and_then(|key| key.get_value::<u32, _>("RemoveWindowsStore").ok())
        == Some(1)
}

fn cfa_mode() -> Option<u32> {
    use winreg::RegKey;
    use winreg::enums::HKEY_LOCAL_MACHINE;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for path in [
        r"SOFTWARE\Policies\Microsoft\Windows Defender\Windows Defender Exploit Guard\Controlled Folder Access",
        r"SOFTWARE\Microsoft\Windows Defender\Windows Defender Exploit Guard\Controlled Folder Access",
    ] {
        if let Some(value) = hklm
            .open_subkey(path)
            .ok()
            .and_then(|key| key.get_value::<u32, _>("EnableControlledFolderAccess").ok())
        {
            return Some(value);
        }
    }
    None
}

fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(Some(0)).collect()
}
