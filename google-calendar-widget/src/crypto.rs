use std::path::Path;
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
};

const ENTROPY: &[u8] = b"gcal-widget-v1";

fn blob_from_slice(data: &[u8]) -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    }
}

fn empty_blob() -> CRYPT_INTEGER_BLOB {
    CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: std::ptr::null_mut(),
    }
}

pub fn protect(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    unsafe {
        let in_blob = blob_from_slice(data);
        let entropy_blob = blob_from_slice(ENTROPY);
        let mut out_blob = empty_blob();

        CryptProtectData(
            &in_blob,
            PCWSTR::null(),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| anyhow::anyhow!("CryptProtectData: {}", e))?;

        if out_blob.pbData.is_null() || out_blob.cbData == 0 {
            anyhow::bail!("CryptProtectData returned empty blob");
        }

        let result =
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut _));
        Ok(result)
    }
}

pub fn unprotect(data: &[u8]) -> anyhow::Result<Vec<u8>> {
    unsafe {
        let in_blob = blob_from_slice(data);
        let entropy_blob = blob_from_slice(ENTROPY);
        let mut out_blob = empty_blob();
        let mut descr = PWSTR::null();

        CryptUnprotectData(
            &in_blob,
            Some(&mut descr),
            Some(&entropy_blob),
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .map_err(|e| anyhow::anyhow!("CryptUnprotectData: {}", e))?;

        if out_blob.pbData.is_null() || out_blob.cbData == 0 {
            anyhow::bail!("CryptUnprotectData returned empty blob");
        }

        let result =
            std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(out_blob.pbData as *mut _));
        if !descr.is_null() {
            let _ = LocalFree(HLOCAL(descr.0 as *mut _));
        }
        Ok(result)
    }
}

pub fn save_encrypted(path: &Path, plaintext: &[u8]) -> anyhow::Result<()> {
    crate::log::write(&format!("crypto: protect {} bytes", plaintext.len()));
    let blob = match protect(plaintext) {
        Ok(b) => b,
        Err(e) => {
            crate::log::write(&format!("crypto: protect failed: {}", e));
            return Err(e);
        }
    };
    crate::log::write(&format!("crypto: protected blob {} bytes", blob.len()));
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            crate::log::write(&format!("crypto: create_dir_all failed: {}", e));
            return Err(e.into());
        }
    }
    match std::fs::write(path, &blob) {
        Ok(()) => {
            crate::log::write(&format!("crypto: wrote {:?}", path));
            Ok(())
        }
        Err(e) => {
            crate::log::write(&format!("crypto: write {:?} failed: {}", path, e));
            Err(e.into())
        }
    }
}

pub fn load_encrypted(path: &Path) -> Option<Vec<u8>> {
    let blob = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => {
            crate::log::write(&format!(
                "crypto: read {:?} failed: {}",
                path, e
            ));
            return None;
        }
    };
    crate::log::write(&format!(
        "crypto: read {:?}, {} bytes",
        path,
        blob.len()
    ));
    match unprotect(&blob) {
        Ok(v) => {
            crate::log::write(&format!("crypto: unprotected {} bytes", v.len()));
            Some(v)
        }
        Err(e) => {
            crate::log::write(&format!("crypto: unprotect failed: {}", e));
            None
        }
    }
}