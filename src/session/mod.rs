//! Session persistence: `%LocalAppData%\\ContainerWatch\\session.json` + DPAPI.

use crate::models::SavedConnectionDto;
use std::fs;
use std::path::PathBuf;

pub struct LoadedSession {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
}

pub fn session_file_path() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| String::new());
    PathBuf::from(base).join("ContainerWatch").join("session.json")
}

pub fn try_load() -> Option<LoadedSession> {
    let path = session_file_path();
    let json = fs::read_to_string(path).ok()?;
    let dto: SavedConnectionDto = serde_json::from_str(&json).ok()?;

    let password = dto
        .password_protected
        .as_ref()
        .and_then(|b64| dpapi_unprotect(b64).ok());

    Some(LoadedSession {
        host: dto.host,
        port: if dto.port > 0 { dto.port } else { 22 },
        username: dto.username,
        password,
    })
}

pub fn save(host: &str, port: u16, username: &str, password: Option<&str>) -> std::io::Result<()> {
    let path = session_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let password_protected = password
        .filter(|p| !p.is_empty())
        .and_then(|p| dpapi_protect(p.as_bytes()).ok());

    let dto = SavedConnectionDto {
        host: host.to_string(),
        port,
        username: username.to_string(),
        password_protected,
    };

    let json = serde_json::to_string_pretty(&dto).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
    })?;
    fs::write(path, json)
}

pub fn clear() {
    let path = session_file_path();
    let _ = fs::remove_file(path);
}

#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use windows::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: data.len() as u32,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &mut input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("DPAPI protect failed: {e}"))?;

        let slice = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let encoded = STANDARD.encode(slice);
        windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
            output.pbData as _,
        ));
        Ok(encoded)
    }
}

#[cfg(windows)]
fn dpapi_unprotect(b64: &str) -> Result<String, String> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use windows::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let cipher = STANDARD
        .decode(b64)
        .map_err(|e| format!("base64 decode failed: {e}"))?;

    let mut input = CRYPT_INTEGER_BLOB {
        cbData: cipher.len() as u32,
        pbData: cipher.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &mut input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| format!("DPAPI unprotect failed: {e}"))?;

        let slice = std::slice::from_raw_parts(output.pbData, output.cbData as usize);
        let plain = String::from_utf8(slice.to_vec())
            .map_err(|e| format!("utf8 decode failed: {e}"))?;
        windows::Win32::Foundation::LocalFree(windows::Win32::Foundation::HLOCAL(
            output.pbData as _,
        ));
        Ok(plain)
    }
}

#[cfg(not(windows))]
fn dpapi_protect(_data: &[u8]) -> Result<String, String> {
    Err("DPAPI is only available on Windows".into())
}

#[cfg(not(windows))]
fn dpapi_unprotect(_b64: &str) -> Result<String, String> {
    Err("DPAPI is only available on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_dto_roundtrip_json() {
        let dto = SavedConnectionDto {
            host: "192.168.1.50".into(),
            port: 22,
            username: "deploy".into(),
            password_protected: Some("abc123".into()),
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("\"Host\""));
        assert!(json.contains("\"PasswordProtected\""));
        let back: SavedConnectionDto = serde_json::from_str(&json).unwrap();
        assert_eq!(back.host, "192.168.1.50");
    }

    #[cfg(windows)]
    #[test]
    fn dpapi_roundtrip() {
        let protected = dpapi_protect(b"secret-pass").unwrap();
        let plain = dpapi_unprotect(&protected).unwrap();
        assert_eq!(plain, "secret-pass");
    }
}
