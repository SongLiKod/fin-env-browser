use std::{
    path::{Path, PathBuf},
    sync::RwLock,
};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use rand::{rngs::OsRng, RngCore};
use ring::{
    aead::{self, Aad, LessSafeKey, Nonce, UnboundKey},
    hkdf::{self, KeyType},
};
use sha2::Sha256;

use crate::{
    error::{AppError, Result},
    paths,
};

const STORED_PREFIX: &str = "v1:";
const PACKAGE_MAGIC: &[u8; 8] = b"FINENV01";
const MASTER_SALT_LEN: usize = 32;
const PASSWORD_ITERATIONS: u32 = 120_000;
const PACKAGE_ITERATIONS: u32 = 210_000;

/// Local AES-256-GCM key manager.
///
/// The master key is derived with HKDF-SHA256 from a random salt stored in the
/// application data directory and this machine's identity (host + user + OS).
/// No key material is hard coded, and the salt is never synced to the cloud.
pub struct CryptoManager {
    key: RwLock<[u8; 32]>,
    device_key: [u8; 32],
    key_file: PathBuf,
    #[allow(dead_code)]
    salt: Vec<u8>,
}

const KEY_FILE_NAME: &str = "master.key.enc";
const MASTER_KEY_AAD: &[u8] = b"finenv-master-key-v1";

impl CryptoManager {
    pub fn initialize(data_dir: &Path) -> Result<Self> {
        paths::ensure_dir(data_dir)?;
        let salt_path = paths::master_salt_path(data_dir);
        let salt = if salt_path.exists() {
            let bytes = std::fs::read(&salt_path)?;
            if bytes.len() < 16 {
                return Err(AppError::internal("主密钥盐文件损坏"));
            }
            bytes
        } else {
            let mut salt = vec![0u8; MASTER_SALT_LEN];
            OsRng.fill_bytes(&mut salt);
            std::fs::write(&salt_path, &salt)?;
            restrict_permissions(&salt_path);
            salt
        };

        let identity = machine_identity();
        let prk = hkdf::Salt::new(hkdf::HKDF_SHA256, &salt).extract(identity.as_bytes());
        struct KeyLen;
        impl KeyType for KeyLen {
            fn len(&self) -> usize {
                32
            }
        }
        let mut device_key = [0u8; 32];
        prk.expand(&[b"fin-env-browser master key v1"], KeyLen)
            .map_err(|_| AppError::internal("无法派生主密钥"))?
            .fill(&mut device_key)
            .map_err(|_| AppError::internal("无法填充主密钥"))?;

        let key_file = data_dir.join(KEY_FILE_NAME);
        let key = if key_file.exists() {
            let bytes = std::fs::read(&key_file)?;
            let plain = open(&device_key, MASTER_KEY_AAD, &bytes)
                .map_err(|_| AppError::internal("本机主密钥文件损坏或设备信息已变化"))?;
            if plain.len() != 32 {
                return Err(AppError::internal("本机主密钥长度无效"));
            }
            let mut key = [0u8; 32];
            key.copy_from_slice(&plain);
            key
        } else {
            device_key
        };

        let manager = Self {
            key: RwLock::new(key),
            device_key,
            key_file,
            salt,
        };
        if !manager.key_file.exists() {
            manager.persist_key(key)?;
        }
        Ok(manager)
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        let key = self.key()?;
        let sealed = seal(&key, b"finenv-field-v1", plaintext.as_bytes())?;
        Ok(format!("{STORED_PREFIX}{}", B64.encode(sealed)))
    }

    pub fn decrypt(&self, stored: &str) -> Result<String> {
        if stored.is_empty() {
            return Ok(String::new());
        }
        let raw = stored
            .strip_prefix(STORED_PREFIX)
            .ok_or_else(|| AppError::internal("密文格式不受支持"))?;
        let bytes = B64
            .decode(raw)
            .map_err(|_| AppError::internal("密文 base64 解码失败"))?;
        let key = self.key()?;
        let plain = open(&key, b"finenv-field-v1", &bytes)?;
        String::from_utf8(plain).map_err(|_| AppError::internal("密文解密后不是 UTF-8"))
    }

    /// Export the raw master key for a passphrase-encrypted portable backup.
    /// The key is only ever placed inside the outer AES-GCM package, never in
    /// plaintext on disk.
    pub fn export_key(&self) -> Result<[u8; 32]> {
        self.key()
    }

    /// Adopt the key from a passphrase-encrypted backup restored on another
    /// machine. Must be called only after the database restore succeeds. The
    /// imported key is immediately wrapped with this machine's device key so
    /// subsequent launches can open the restored data.
    pub fn import_key(&self, key: [u8; 32]) -> Result<()> {
        if key.iter().all(|byte| *byte == 0) {
            return Err(AppError::internal("拒绝导入全零主密钥"));
        }
        self.persist_key(key)?;
        let mut guard = self
            .key
            .write()
            .map_err(|_| AppError::internal("主密钥锁不可用"))?;
        *guard = key;
        Ok(())
    }

    fn persist_key(&self, key: [u8; 32]) -> Result<()> {
        let sealed = seal(&self.device_key, MASTER_KEY_AAD, &key)?;
        std::fs::write(&self.key_file, sealed)?;
        restrict_permissions(&self.key_file);
        Ok(())
    }

    fn key(&self) -> Result<[u8; 32]> {
        self.key
            .read()
            .map(|guard| *guard)
            .map_err(|_| AppError::internal("主密钥锁不可用"))
    }

    pub fn hash_password(&self, password: &str) -> Result<String> {
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        let mut hash = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, PASSWORD_ITERATIONS, &mut hash);
        Ok(format!(
            "pbkdf2-sha256${PASSWORD_ITERATIONS}${}${}",
            hex::encode(salt),
            hex::encode(hash)
        ))
    }

    pub fn verify_password(&self, password: &str, stored: &str) -> bool {
        let mut parts = stored.split('$');
        if parts.next() != Some("pbkdf2-sha256") {
            return false;
        }
        let iterations: u32 = match parts.next().and_then(|v| v.parse().ok()) {
            Some(v) => v,
            None => return false,
        };
        let salt = match parts.next().and_then(|v| hex::decode(v).ok()) {
            Some(v) => v,
            None => return false,
        };
        let expected = match parts.next().and_then(|v| hex::decode(v).ok()) {
            Some(v) => v,
            None => return false,
        };
        let mut actual = [0u8; 32];
        pbkdf2::pbkdf2_hmac::<Sha256>(password.as_bytes(), &salt, iterations, &mut actual);
        constant_time_eq(&actual, &expected)
    }

    /// Encrypt a portable package (environment export, backup, cloud sync).
    /// The whole payload is encrypted with a key derived from the passphrase, so
    /// neither cloud storage nor another machine can read it without it.
    pub fn encrypt_package(&self, plaintext: &[u8], passphrase: &str) -> Result<Vec<u8>> {
        if passphrase.chars().count() < 6 {
            return Err(AppError::bad_request("加密口令至少需要 6 个字符"));
        }
        let mut salt = [0u8; 16];
        OsRng.fill_bytes(&mut salt);
        let key = derive_passphrase_key(passphrase, &salt, PACKAGE_ITERATIONS)?;
        let sealed = seal(&key, b"finenv-package-v1", plaintext)?;
        let mut out = Vec::with_capacity(9 + 16 + sealed.len());
        out.extend_from_slice(PACKAGE_MAGIC);
        out.push(b'E');
        out.extend_from_slice(&salt);
        out.extend_from_slice(&sealed);
        Ok(out)
    }

    pub fn decrypt_package(&self, data: &[u8], passphrase: &str) -> Result<Vec<u8>> {
        if data.len() < 9 + 16 + 12 + 16 || &data[..8] != PACKAGE_MAGIC || data[8] != b'E' {
            return Err(AppError::bad_request("不是有效的 FinEnv 加密包"));
        }
        let salt = &data[9..25];
        let key = derive_passphrase_key(passphrase, salt, PACKAGE_ITERATIONS)?;
        open(&key, b"finenv-package-v1", &data[25..])
            .map_err(|_| AppError::bad_request("口令错误或加密包已损坏"))
    }

    pub fn encrypt_package_if_needed(
        &self,
        plaintext: &[u8],
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>> {
        match passphrase.filter(|p| !p.trim().is_empty()) {
            Some(pass) => self.encrypt_package(plaintext, pass),
            None => {
                let mut out = Vec::with_capacity(9 + plaintext.len());
                out.extend_from_slice(PACKAGE_MAGIC);
                out.push(b'P');
                out.extend_from_slice(plaintext);
                Ok(out)
            }
        }
    }

    pub fn decrypt_package_if_needed(
        &self,
        data: &[u8],
        passphrase: Option<&str>,
    ) -> Result<Vec<u8>> {
        if data.len() >= 9 && &data[..8] == PACKAGE_MAGIC {
            match data[8] {
                b'P' => return Ok(data[9..].to_vec()),
                b'E' => {
                    let pass = passphrase
                        .filter(|p| !p.trim().is_empty())
                        .ok_or_else(|| AppError::bad_request("该文件已加密，请提供口令"))?;
                    return self.decrypt_package(data, pass);
                }
                _ => return Err(AppError::bad_request("FinEnv 包标志位未知")),
            }
        }
        Err(AppError::bad_request("不是有效的 FinEnv 包文件"))
    }
}

fn seal(key: &[u8; 32], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let unbound =
        UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| AppError::internal("AES 密钥无效"))?;
    let key = LessSafeKey::new(unbound);
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut out = plaintext.to_vec();
    key.seal_in_place_append_tag(nonce, Aad::from(aad), &mut out)
        .map_err(|_| AppError::internal("AES-GCM 加密失败"))?;
    let mut result = Vec::with_capacity(12 + out.len());
    result.extend_from_slice(&nonce_bytes);
    result.extend_from_slice(&out);
    Ok(result)
}

fn open(key: &[u8; 32], aad: &[u8], data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 12 + 16 {
        return Err(AppError::internal("密文长度不足"));
    }
    let mut nonce_bytes = [0u8; 12];
    nonce_bytes.copy_from_slice(&data[..12]);
    let unbound =
        UnboundKey::new(&aead::AES_256_GCM, key).map_err(|_| AppError::internal("AES 密钥无效"))?;
    let key = LessSafeKey::new(unbound);
    let nonce = Nonce::assume_unique_for_key(nonce_bytes);
    let mut buf = data[12..].to_vec();
    let plain = key
        .open_in_place(nonce, Aad::from(aad), &mut buf)
        .map_err(|_| AppError::internal("AES-GCM 解密失败"))?;
    Ok(plain.to_vec())
}

fn derive_passphrase_key(passphrase: &str, salt: &[u8], iterations: u32) -> Result<[u8; 32]> {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<Sha256>(passphrase.as_bytes(), salt, iterations, &mut key);
    Ok(key)
}

fn machine_identity() -> String {
    let host = whoami::fallible::hostname().unwrap_or_else(|_| "unknown-host".into());
    let user = whoami::fallible::username().unwrap_or_else(|_| "unknown-user".into());
    let os = std::env::consts::OS;
    let arch = std::env::consts::ARCH;
    let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
    let machine_guid = read_windows_machine_guid().unwrap_or_default();
    format!("{host}|{user}|{os}|{arch}|{computer}|{machine_guid}|finenv-key-v1")
}

#[cfg(target_os = "windows")]
fn read_windows_machine_guid() -> Option<String> {
    use std::process::Command;
    let output = Command::new("reg")
        .args([
            "query",
            "HKLM\\SOFTWARE\\Microsoft\\Cryptography",
            "/v",
            "MachineGuid",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    text.lines()
        .find(|l| l.contains("MachineGuid"))
        .and_then(|l| l.split_whitespace().last())
        .map(|s| s.trim().to_string())
}

#[cfg(not(target_os = "windows"))]
fn read_windows_machine_guid() -> Option<String> {
    std::fs::read_to_string("/etc/machine-id")
        .ok()
        .map(|s| s.trim().to_string())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    if let Ok(meta) = std::fs::metadata(path) {
        let mut perms = meta.permissions();
        perms.set_mode(0o600);
        let _ = std::fs::set_permissions(path, perms);
    }
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_round_trip() {
        let dir = std::env::temp_dir().join(format!("finenv-crypto-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let crypto = CryptoManager::initialize(&dir).unwrap();
        let enc = crypto.encrypt("秘密 password").unwrap();
        assert!(!enc.contains("秘密"));
        assert_eq!(crypto.decrypt(&enc).unwrap(), "秘密 password");
        assert!(crypto.verify_password("hello", &crypto.hash_password("hello").unwrap()));
    }

    #[test]
    fn package_round_trip() {
        let dir = std::env::temp_dir().join(format!("finenv-crypto-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let crypto = CryptoManager::initialize(&dir).unwrap();
        let data = br#"{"hello":"world"}"#;
        let enc = crypto.encrypt_package(data, "passphrase").unwrap();
        assert_eq!(crypto.decrypt_package(&enc, "passphrase").unwrap(), data);
        assert!(crypto.decrypt_package(&enc, "wrong-pass").is_err());
    }

    #[test]
    fn portable_master_key_can_be_imported_and_persists() {
        let source = std::env::temp_dir().join(format!("finenv-key-a-{}", uuid::Uuid::new_v4()));
        let target = std::env::temp_dir().join(format!("finenv-key-b-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&target).unwrap();
        let source_crypto = CryptoManager::initialize(&source).unwrap();
        let imported_key = [0x42u8; 32];
        source_crypto.import_key(imported_key).unwrap();
        let encrypted = source_crypto.encrypt("cross-device secret").unwrap();
        let key = source_crypto.export_key().unwrap();
        let target_crypto = CryptoManager::initialize(&target).unwrap();
        target_crypto.import_key(key).unwrap();
        assert_eq!(
            target_crypto.decrypt(&encrypted).unwrap(),
            "cross-device secret"
        );
        drop(target_crypto);
        let reopened = CryptoManager::initialize(&target).unwrap();
        assert_eq!(reopened.decrypt(&encrypted).unwrap(), "cross-device secret");
    }
}
