use std::collections::HashMap;
use std::sync::Arc;

use once_cell::sync::Lazy;
use sha1::{Digest, Sha1};
use tokio::sync::{Mutex, OwnedMutexGuard};

// 同一仓库分支的写操作串行化，避免并发建文件冲突
static WRITE_LOCKS: Lazy<std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>> = Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

pub async fn write_guard(provider: &str, key: &str) -> OwnedMutexGuard<()> {
    let lock_key = format!("{}:{}", provider, key);
    let lock = {
        let mut locks = WRITE_LOCKS.lock().expect("repository write lock poisoned");
        locks.entry(lock_key).or_insert_with(|| Arc::new(Mutex::new(()))).clone()
    };
    lock.lock_owned().await
}

// 计算与 Git 一致的 blob SHA1，用于判断已存在文件与待上传内容是否一致
pub fn git_blob_sha1(data: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(format!("blob {}\0", data.len()).as_bytes());
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::git_blob_sha1;

    #[test]
    fn git_blob_sha1_matches_git() {
        assert_eq!(git_blob_sha1(b""), "e69de29bb2d1d6434b8b29ae775ad8c2e48c5391");
        assert_eq!(git_blob_sha1(b"hello world\n"), "3b18e512dba79e4c8300dd08aeb37f8e728b8dad");
    }
}
