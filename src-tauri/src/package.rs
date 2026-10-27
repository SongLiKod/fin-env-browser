use std::{
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::{Deserialize, Serialize};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    crypto::CryptoManager,
    db::Db,
    error::{AppError, Result},
    fingerprint,
    models::{
        new_id, now_iso, Account, AccountExportRow, CookieRecord, Environment, FingerprintSnapshot,
        HostsEntryInput, MockRuleInput,
    },
    paths,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyPackage {
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentPackage {
    pub environment: Environment,
    pub accounts: Vec<AccountExportRow>,
    pub snapshots: Vec<FingerprintSnapshot>,
    pub proxy: Option<ProxyPackage>,
    pub hosts: Vec<HostsEntryInput>,
    pub mocks: Vec<MockRuleInput>,
    #[serde(default)]
    pub cookies: Vec<CookieRecord>,
    #[serde(default)]
    pub profile_archive: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentBundle {
    pub format_version: u32,
    pub exported_at: String,
    pub environments: Vec<EnvironmentPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPackageEntry {
    pub env_id: String,
    pub env_name: String,
    pub username: String,
    pub password: String,
    pub remark: String,
    pub owner: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountPackage {
    pub format_version: u32,
    pub exported_at: String,
    pub accounts: Vec<AccountPackageEntry>,
}

pub fn export_environments(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    ids: &[String],
    path: &Path,
    passphrase: &str,
    include_profiles: bool,
) -> Result<usize> {
    if ids.is_empty() {
        return Err(AppError::bad_request("请选择至少一个环境"));
    }
    let mut packages = Vec::new();
    for id in ids {
        packages.push(build_environment_package(
            db,
            crypto,
            data_dir,
            id,
            include_profiles,
        )?);
    }
    let bundle = EnvironmentBundle {
        format_version: 1,
        exported_at: now_iso(),
        environments: packages,
    };
    let json = serde_json::to_vec(&bundle)?;
    let encrypted = crypto.encrypt_package(&json, passphrase)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, encrypted)?;
    Ok(bundle.environments.len())
}

fn build_environment_package(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    id: &str,
    include_profiles: bool,
) -> Result<EnvironmentPackage> {
    let env = db.get_environment(id)?;
    let accounts = db
        .list_accounts(id)?
        .into_iter()
        .map(|account| {
            let password = if account.password_enc.is_empty() {
                String::new()
            } else {
                crypto.decrypt(&account.password_enc).unwrap_or_default()
            };
            AccountExportRow {
                username: account.username,
                password,
                remark: account.remark,
                owner: account.owner,
                status: account.status,
            }
        })
        .collect();
    let snapshots = db.list_snapshots(id)?;
    let proxy = match &env.proxy_id {
        Some(proxy_id) => {
            let proxy = db.get_proxy(proxy_id)?;
            let password = if proxy.password_enc.is_empty() {
                String::new()
            } else {
                crypto.decrypt(&proxy.password_enc)?
            };
            Some(ProxyPackage {
                name: proxy.name,
                protocol: proxy.protocol,
                host: proxy.host,
                port: proxy.port,
                username: proxy.username,
                password,
                enabled: proxy.enabled,
            })
        }
        None => None,
    };
    let hosts = db
        .list_hosts(id)?
        .into_iter()
        .map(|h| HostsEntryInput {
            host: h.host,
            ip: h.ip,
            enabled: Some(h.enabled),
        })
        .collect();
    let mocks = db
        .list_mocks(id)?
        .into_iter()
        .map(|m| MockRuleInput {
            pattern: m.pattern,
            method: m.method,
            status_code: m.status_code,
            headers: m.headers,
            body: m.body,
            delay_ms: m.delay_ms,
            enabled: Some(m.enabled),
        })
        .collect();
    let cookies = match db.load_cookie_cache(id)? {
        Some(encrypted) => crypto
            .decrypt(&encrypted)
            .ok()
            .and_then(|plain| serde_json::from_str::<Vec<CookieRecord>>(&plain).ok())
            .unwrap_or_default(),
        None => Vec::new(),
    };
    let profile_archive = if include_profiles {
        let dir = if env.profile_dir.trim().is_empty() {
            paths::env_profile_dir(data_dir, id)
        } else {
            PathBuf::from(&env.profile_dir)
        };
        if dir.exists() {
            Some(zip_profile(&dir)?)
        } else {
            None
        }
    } else {
        None
    };
    Ok(EnvironmentPackage {
        environment: env,
        accounts,
        snapshots,
        proxy,
        hosts,
        mocks,
        cookies,
        profile_archive,
    })
}

pub fn import_environments(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    path: &Path,
    passphrase: &str,
) -> Result<Vec<Environment>> {
    let bytes = std::fs::read(path)?;
    let plain = crypto.decrypt_package(&bytes, passphrase)?;
    let bundle: EnvironmentBundle = serde_json::from_slice(&plain)
        .map_err(|e| AppError::bad_request(format!("环境包内容无效: {e}")))?;
    if bundle.environments.is_empty() {
        return Err(AppError::bad_request("环境包中没有环境"));
    }
    let mut imported = Vec::new();
    for package in bundle.environments {
        imported.push(import_one(db, crypto, data_dir, package)?);
    }
    Ok(imported)
}

fn import_one(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    package: EnvironmentPackage,
) -> Result<Environment> {
    let mut env = package.environment;
    let validation = fingerprint::validate(&env.fingerprint);
    if !validation.valid {
        let messages = validation
            .errors
            .iter()
            .map(|i| format!("{}: {}", i.field, i.message))
            .collect::<Vec<_>>()
            .join("; ");
        return Err(AppError::bad_request(format!(
            "导入环境 {} 的指纹不一致: {messages}",
            env.name
        )));
    }
    let original_name = env.name.clone();
    let mut name = original_name.clone();
    let mut index = 2;
    while db.find_environment_by_name(&name)?.is_some() {
        name = format!("{original_name} (导入 {index})");
        index += 1;
    }
    env.id = new_id();
    env.name = name;
    env.profile_dir = paths::env_profile_dir(data_dir, &env.id)
        .to_string_lossy()
        .to_string();
    env.fingerprint_snapshot_id = None;
    env.created_at = now_iso();
    env.updated_at = now_iso();
    env.last_opened_at = None;
    env.status = "active".into();
    env.ip_address = None;
    env.ip_country = None;
    env.ip_country_code = None;
    env.ip_city = None;
    env.ip_timezone = None;
    env.ip_source = None;
    env.ip_checked_at = None;

    if let Some(proxy) = package.proxy {
        let existing = db.list_proxies()?.into_iter().find(|p| {
            p.protocol == proxy.protocol
                && p.host == proxy.host
                && p.port == proxy.port
                && p.username == proxy.username
        });
        env.proxy_id = Some(match existing {
            Some(proxy) => proxy.id,
            None => {
                let id = new_id();
                let now = now_iso();
                let proxy_record = crate::models::Proxy {
                    id: id.clone(),
                    name: proxy.name,
                    protocol: proxy.protocol,
                    host: proxy.host,
                    port: proxy.port,
                    username: proxy.username,
                    password_enc: if proxy.password.is_empty() {
                        String::new()
                    } else {
                        crypto.encrypt(&proxy.password)?
                    },
                    has_password: !proxy.password.is_empty(),
                    enabled: proxy.enabled,
                    last_check_at: None,
                    latency_ms: None,
                    exit_ip: None,
                    exit_country: None,
                    exit_timezone: None,
                    created_at: now.clone(),
                    updated_at: now,
                };
                db.insert_proxy(&proxy_record)?;
                id
            }
        });
    } else {
        env.proxy_id = None;
    }

    paths::ensure_dir(Path::new(&env.profile_dir))?;
    db.insert_environment(&env)?;
    let mut accounts = package.accounts;
    for account in &mut accounts {
        if account.username.trim().is_empty() {
            continue;
        }
        let now = now_iso();
        let record = Account {
            id: new_id(),
            env_id: env.id.clone(),
            username: account.username.trim().to_string(),
            password_enc: if account.password.is_empty() {
                String::new()
            } else {
                crypto.encrypt(&account.password)?
            },
            has_password: !account.password.is_empty(),
            remark: account.remark.clone(),
            owner: account.owner.clone(),
            status: if crate::models::ACCOUNT_STATUSES.contains(&account.status.as_str()) {
                account.status.clone()
            } else {
                "pending".into()
            },
            created_at: now.clone(),
            updated_at: now,
        };
        db.insert_account(&record)?;
    }
    for mut snapshot in package.snapshots {
        snapshot.id = new_id();
        snapshot.env_id = env.id.clone();
        snapshot.create_time = now_iso();
        db.insert_snapshot(&snapshot)?;
    }
    db.replace_hosts(&env.id, &package.hosts)?;
    db.replace_mocks(&env.id, &package.mocks)?;
    if !package.cookies.is_empty() {
        let raw = serde_json::to_string(&package.cookies)?;
        let encrypted = crypto.encrypt(&raw)?;
        db.save_cookie_cache(&env.id, &encrypted)?;
    }
    if let Some(archive) = package.profile_archive {
        extract_profile(&archive, Path::new(&env.profile_dir))?;
    }
    Ok(env)
}

fn zip_profile(dir: &Path) -> Result<String> {
    let mut bytes = Cursor::new(Vec::<u8>::new());
    {
        let mut zip = ZipWriter::new(&mut bytes);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        let mut stack = vec![dir.to_path_buf()];
        while let Some(current) = stack.pop() {
            for entry in std::fs::read_dir(&current)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    let relative = path
                        .strip_prefix(dir)
                        .map_err(|_| AppError::internal("profile 路径解析失败"))?;
                    let name = relative.to_string_lossy().replace('\\', "/");
                    zip.start_file(name, options)
                        .map_err(|e| AppError::internal(format!("打包 profile 失败: {e}")))?;
                    zip.write_all(&std::fs::read(&path)?)?;
                }
            }
        }
        zip.finish()
            .map_err(|e| AppError::internal(format!("完成 profile 打包失败: {e}")))?;
    }
    Ok(B64.encode(bytes.into_inner()))
}

fn extract_profile(archive: &str, target: &Path) -> Result<u32> {
    let bytes = B64
        .decode(archive)
        .map_err(|_| AppError::bad_request("profile 压缩包 base64 无效"))?;
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| AppError::bad_request(format!("profile 压缩包无效: {e}")))?;
    let mut count = 0;
    for i in 0..zip.len() {
        let mut entry = zip
            .by_index(i)
            .map_err(|e| AppError::internal(format!("读取 profile 失败: {e}")))?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        if name.contains("..") || Path::new(&name).is_absolute() {
            continue;
        }
        let path = target.join(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer)?;
        std::fs::write(path, buffer)?;
        count += 1;
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Account export / import
// ---------------------------------------------------------------------------

pub fn export_accounts(
    db: &Db,
    crypto: &CryptoManager,
    env_ids: &[String],
    path: &Path,
    passphrase: &str,
) -> Result<usize> {
    let mut entries = Vec::new();
    for env_id in env_ids {
        let env = db.get_environment(env_id)?;
        for account in db.list_accounts(env_id)? {
            let password = if account.password_enc.is_empty() {
                String::new()
            } else {
                crypto.decrypt(&account.password_enc)?
            };
            entries.push(AccountPackageEntry {
                env_id: env.id.clone(),
                env_name: env.name.clone(),
                username: account.username,
                password,
                remark: account.remark,
                owner: account.owner,
                status: account.status,
            });
        }
    }
    let package = AccountPackage {
        format_version: 1,
        exported_at: now_iso(),
        accounts: entries,
    };
    let encrypted = crypto.encrypt_package(&serde_json::to_vec(&package)?, passphrase)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, encrypted)?;
    Ok(package.accounts.len())
}

pub fn import_account_rows(
    db: &Db,
    crypto: &CryptoManager,
    env_id: &str,
    rows: &[AccountExportRow],
) -> Result<u32> {
    let mut records = Vec::new();
    let now = now_iso();
    let mut seen = std::collections::HashSet::new();
    for row in rows {
        let username = row.username.trim();
        if username.is_empty() || !seen.insert(username.to_string()) {
            continue;
        }
        records.push(Account {
            id: new_id(),
            env_id: env_id.to_string(),
            username: username.to_string(),
            password_enc: if row.password.is_empty() {
                String::new()
            } else {
                crypto.encrypt(&row.password)?
            },
            has_password: !row.password.is_empty(),
            remark: row.remark.clone(),
            owner: row.owner.clone(),
            status: if crate::models::ACCOUNT_STATUSES.contains(&row.status.as_str()) {
                row.status.clone()
            } else {
                "pending".into()
            },
            created_at: now.clone(),
            updated_at: now.clone(),
        });
    }
    db.bulk_insert_accounts(&records)
}

pub fn import_accounts_file(
    db: &Db,
    crypto: &CryptoManager,
    env_id: &str,
    path: &Path,
    passphrase: &str,
) -> Result<u32> {
    let bytes = std::fs::read(path)?;
    let plain = crypto.decrypt_package(&bytes, passphrase)?;
    let package: AccountPackage = serde_json::from_slice(&plain)
        .map_err(|e| AppError::bad_request(format!("账号包内容无效: {e}")))?;
    let rows: Vec<AccountExportRow> = package
        .accounts
        .into_iter()
        .map(|a| AccountExportRow {
            username: a.username,
            password: a.password,
            remark: a.remark,
            owner: a.owner,
            status: a.status,
        })
        .collect();
    import_account_rows(db, crypto, env_id, &rows)
}
