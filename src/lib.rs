use anyhow::Error;
use bb8::{Pool, PooledConnection};
use bb8_redis::RedisConnectionManager;
use lru::LruCache;
use redis::AsyncCommands;
use serde::{de::DeserializeOwned, Serialize};
use std::{num::NonZeroUsize, sync::Arc};
use tokio::sync::RwLock;

pub type Result<T = (), E = Error> = std::result::Result<T, E>;

#[derive(Clone)]
pub struct CacheClient {
    pool: Pool<RedisConnectionManager>,
    json_cache: Arc<RwLock<LruCache<String, String>>>, // In-memory cache for JSON values
}

impl CacheClient {
    /// Initializes a new CacheClient with a connection pool and an optional in-memory cache.
    pub async fn new(
        url: &str,
        db_index: Option<u8>,
        cache_capacity: usize,
        connection_size: u32,
    ) -> Result<Self> {
        let redis_url = match db_index {
            Some(db) => format!("{}/{}", url.trim_end_matches('/'), db),
            None => url.to_string(),
        };

        let manager = RedisConnectionManager::new(redis_url)?;
        let pool = Pool::builder()
            .max_size(connection_size)
            .build(manager)
            .await?;

        // 设置 LRU 缓存容量
        let json_cache = LruCache::new(NonZeroUsize::new(cache_capacity).unwrap());

        Ok(Self {
            pool,
            json_cache: Arc::new(RwLock::new(json_cache)),
        })
    }

    /// Gets a pooled Redis connection.
    async fn get_conn(&self) -> Result<PooledConnection<'_, RedisConnectionManager>> {
        self.pool
            .get()
            .await
            .map_err(|e| Error::msg(format!("Failed to get Redis connection from pool: {}", e)))
    }

    /// 将 JSON 数据写入缓存，并同时写入 Redis
    pub async fn set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let serialized = serde_json::to_string(value)?;

        // 插入到 LRU 缓存中
        self.json_cache
            .write()
            .await
            .put(key.to_string(), serialized.clone());

        // 写入 Redis
        self.set(key, &serialized).await
    }

    /// 从缓存或 Redis 中获取 JSON 数据
    /// 从缓存或 Redis 中获取 JSON 数据
    pub async fn get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        // 尝试从缓存中读取
        // 尝试从缓存中读取
        {
            let mut json_cache = self.json_cache.write().await; // 改为写锁
            if let Some(cached_value) = json_cache.get_mut(key) {
                // 使用 get_mut
                return serde_json::from_str(cached_value).map(Some).map_err(|e| {
                    Error::msg(format!(
                        "Failed to deserialize cached JSON for key '{}': {}",
                        key, e
                    ))
                });
            }
        } // 锁在这里释放

        // 如果缓存中没有，尝试从 Redis 获取
        if let Some(serialized) = self.get(key).await? {
            let value: T = serde_json::from_str(&serialized).map_err(|e| {
                Error::msg(format!(
                    "Failed to deserialize JSON from Redis for key '{}': {}",
                    key, e
                ))
            })?;

            // 将获取到的数据存入缓存
            self.json_cache
                .write()
                .await
                .put(key.to_string(), serialized);

            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// 设置键值对，带过期时间
    pub async fn set_ex(&self, key: &str, value: &str, sec: u64) -> Result<()> {
        let mut conn = self.get_conn().await?;
        conn.set_ex(key, value, sec).await.map_err(Error::from)
    }

    /// 设置键值对，不带过期时间
    pub async fn set(&self, key: &str, value: &str) -> Result<()> {
        let mut conn = self.get_conn().await?;
        conn.set(key, value).await.map_err(Error::from)
    }

    /// 将 JSON 数据写入缓存并设置过期时间
    pub async fn set_json_ex<T: Serialize>(&self, key: &str, value: &T, sec: u64) -> Result<()> {
        let serialized = serde_json::to_string(value)?;

        // 将序列化后的数据写入 LRU 缓存
        self.json_cache
            .write()
            .await
            .put(key.to_string(), serialized.clone());

        // 将数据写入 Redis，并设置过期时间
        self.set_ex(key, &serialized, sec).await
    }

    /// Get a key's value.
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let mut conn = self.get_conn().await?;
        conn.get(key).await.map_err(Error::from)
    }

    /// Check if a key exists.
    pub async fn exists(&self, key: &str) -> Result<bool> {
        let mut conn = self.get_conn().await?;
        conn.exists(key).await.map_err(Error::from)
    }

    /// Delete a key.
    pub async fn delete(&self, key: &str) -> Result<()> {
        let mut json_cache = self.json_cache.write().await; // 获取可变引用
        json_cache.demote(key);
        let mut conn = self.get_conn().await?;
        conn.del(key).await.map_err(Error::from)
    }

    /// Get all keys matching a pattern.
    pub async fn keys(&self, pattern: &str) -> Result<Vec<String>> {
        let mut conn = self.get_conn().await?;
        conn.keys(pattern).await.map_err(Error::from)
    }

    /// Add JSON data to cache with a 60-second expiration time.
    pub async fn add_cache_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        self.set_json_ex(key, value, 60).await
    }

    /// Get JSON data from cache.
    pub async fn get_cache_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        self.get_json(key).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;
    use tokio::time::sleep;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        field1: String,
        field2: i32,
    }

    async fn get_client() -> CacheClient {
        CacheClient::new("redis://:123456@127.0.0.1/", None, 1000, 15)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_set_and_get() {
        let client = get_client().await;
        let key = "test_key";
        let value = "test_value";

        client.set(key, value).await.unwrap();
        let result: Option<String> = client.get(key).await.unwrap();
        assert_eq!(result, Some(value.to_string()));

        client.delete(key).await.unwrap();
    }

    #[tokio::test]
    async fn test_set_ex_and_expire() {
        let client = get_client().await;
        let key = "test_key_ex";
        let value = "test_value_ex";

        client.set_ex(key, value, 1).await.unwrap();
        let result: Option<String> = client.get(key).await.unwrap();
        assert_eq!(result, Some(value.to_string()));

        // Wait for the key to expire
        sleep(Duration::from_secs(2)).await;
        let expired_result: Option<String> = client.get(key).await.unwrap();
        assert_eq!(expired_result, None);
    }

    #[tokio::test]
    async fn test_set_and_get_json() {
        let client = get_client().await;
        let key = "test_json_key";
        let data = TestData {
            field1: "test".to_string(),
            field2: 123,
        };

        client.set_json(key, &data).await.unwrap();
        let result: Option<TestData> = client.get_json(key).await.unwrap();
        assert_eq!(result, Some(data));

        client.delete(key).await.unwrap();
    }

    #[tokio::test]
    async fn test_exists() {
        let client = get_client().await;
        let key = "test_exists_key";

        client.set(key, "value").await.unwrap();
        let exists = client.exists(key).await.unwrap();
        assert!(exists);

        client.delete(key).await.unwrap();
        let exists_after_delete = client.exists(key).await.unwrap();
        assert!(!exists_after_delete);
    }

    #[tokio::test]
    async fn test_keys_pattern() {
        let client = get_client().await;
        let key1 = "test_key_pattern_1";
        let key2 = "test_key_pattern_2";

        client.set(key1, "value1").await.unwrap();
        client.set(key2, "value2").await.unwrap();

        let keys = client.keys("test_key_pattern_*").await.unwrap();
        assert!(keys.contains(&key1.to_string()));
        assert!(keys.contains(&key2.to_string()));

        client.delete(key1).await.unwrap();
        client.delete(key2).await.unwrap();
    }
}
