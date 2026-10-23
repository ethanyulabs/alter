//! 读 `Cookies`（SQLite 副本）并解密成可写进 partition 的 Cookie。
//! macOS 上的 Chromium：`key = PBKDF2-HMAC-SHA1(Safe Storage 口令, "saltysalt", 1003 次, 16 字节)`，
//! `encrypted_value` 是 `v10` + AES-128-CBC（IV 为 16 个空格，PKCS#7）；`meta.version` ≥24 时明文前 32 字节是
//! `SHA256(host_key)`。解密后的值只在内存里。

use alter_core::protocol::{ImportedCookie, SameSite};
use cbc::cipher::{BlockModeDecrypt, KeyIvInit, block_padding::Pkcs7};
use sha2::Digest;

use super::history::chrome_to_unix_ms;

/// 从 Safe Storage 口令派生 AES-128 密钥。
pub(super) fn derive_key(password: &[u8]) -> [u8; 16] {
    let mut key = [0u8; 16];
    pbkdf2::pbkdf2_hmac::<sha1::Sha1>(password, b"saltysalt", 1003, &mut key);
    key
}

/// 解出一个 Cookie 的值；无法解密、主机名前缀不符或不是 UTF-8 时返回 None（跳过）。
/// `encrypted` 为空时用 `value` 列的明文。
pub(super) fn decrypt(
    key: &[u8; 16],
    db_version: i64,
    host_key: &str,
    value: &str,
    encrypted: &[u8],
) -> Option<String> {
    if encrypted.is_empty() {
        return Some(value.to_owned());
    }
    let data = encrypted.strip_prefix(b"v10")?;
    let mut plain = cbc::Decryptor::<aes::Aes128>::new(key.into(), &[b' '; 16].into())
        .decrypt_padded_vec::<Pkcs7>(data)
        .ok()?;
    if db_version >= 24 {
        if plain.len() < 32 || plain[..32] != sha2::Sha256::digest(host_key.as_bytes())[..] {
            return None;
        }
        plain.drain(..32);
    }
    String::from_utf8(plain).ok()
}

/// 读内存里的 Cookies 库 `conn`，取出在 `now_ms`（UTC 毫秒）时仍有效的 Cookie 并解密：返回 (Cookie, 跳过数)。
/// 过期的不导入也不计入跳过。
pub(super) fn read(
    conn: &rusqlite::Connection,
    key: &[u8; 16],
    now_ms: i64,
) -> Result<(Vec<ImportedCookie>, u32), String> {
    let version: i64 = conn
        .query_row("SELECT value FROM meta WHERE key = 'version'", [], |r| {
            r.get::<_, String>(0)
        })
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    let mut stmt = conn
        .prepare(
            "SELECT host_key, name, value, encrypted_value, path, expires_utc, is_secure,
                    is_httponly, samesite, has_expires FROM cookies",
        )
        .map_err(|e| format!("读取 Cookies 失败：{e}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok(Row {
                host: r.get(0)?,
                name: r.get(1)?,
                value: r.get(2)?,
                encrypted: r.get(3)?,
                path: r.get(4)?,
                expires_utc: r.get(5)?,
                secure: r.get::<_, i64>(6)? != 0,
                http_only: r.get::<_, i64>(7)? != 0,
                same_site: r.get(8)?,
                has_expires: r.get::<_, i64>(9)? != 0,
            })
        })
        .map_err(|e| format!("读取 Cookies 失败：{e}"))?;
    let mut out = Vec::new();
    let mut skipped = 0;
    for row in rows {
        let Ok(row) = row else {
            skipped += 1;
            continue;
        };
        let expires_ms = row.has_expires.then(|| chrome_to_unix_ms(row.expires_utc));
        if expires_ms.is_some_and(|t| t <= now_ms) {
            continue;
        }
        let Some(value) = decrypt(key, version, &row.host, &row.value, &row.encrypted) else {
            skipped += 1;
            continue;
        };
        let scheme = if row.secure { "https" } else { "http" };
        out.push(ImportedCookie {
            url: format!(
                "{scheme}://{}{}",
                row.host.trim_start_matches('.'),
                row.path
            ),
            name: row.name,
            value,
            domain: row.host,
            path: row.path,
            secure: row.secure,
            http_only: row.http_only,
            same_site: same_site(row.same_site),
            expiration_date: expires_ms.map(|t| t.div_euclid(1000)),
        });
    }
    Ok((out, skipped))
}

struct Row {
    host: String,
    name: String,
    value: String,
    encrypted: Vec<u8>,
    path: String,
    expires_utc: i64,
    secure: bool,
    http_only: bool,
    same_site: i64,
    has_expires: bool,
}

/// Chromium 的 `samesite` 列：-1 unspecified、0 no_restriction、1 lax、2 strict。
fn same_site(v: i64) -> SameSite {
    match v {
        0 => SameSite::NoRestriction,
        1 => SameSite::Lax,
        2 => SameSite::Strict,
        _ => SameSite::Unspecified,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 没有 `v10` 前缀的非空密文（例如 Linux 的 v11）跳过：即使去掉前缀后的内容能用这把密钥解开，也不导入。
    #[test]
    fn non_v10_ciphertext_is_skipped() {
        use cbc::cipher::BlockModeEncrypt;
        let key = derive_key(b"peanuts");
        let ct = cbc::Encryptor::<aes::Aes128>::new(&key.into(), &[b' '; 16].into())
            .encrypt_padded_vec::<Pkcs7>(b"x");
        let mut v10 = b"v10".to_vec();
        v10.extend_from_slice(&ct);
        assert_eq!(
            decrypt(&key, 0, "a.example", "", &v10),
            Some("x".to_owned())
        );
        let mut v11 = b"v11".to_vec();
        v11.extend_from_slice(&ct);
        assert_eq!(decrypt(&key, 0, "a.example", "", &v11), None);
        assert_eq!(decrypt(&key, 0, "a.example", "", b"plain"), None);
    }

    /// 空密文用 `value` 列的明文，空值也照样导入。
    #[test]
    fn empty_ciphertext_uses_value_column() {
        let key = derive_key(b"peanuts");
        assert_eq!(
            decrypt(&key, 24, "a.example", "p1", b""),
            Some("p1".to_owned())
        );
        assert_eq!(decrypt(&key, 24, "a.example", "", b""), Some(String::new()));
    }
}
