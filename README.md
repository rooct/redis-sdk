- `CacheClient` 代码的 `README.md` 示例。这个文档涵盖了库的基本介绍、安装、使用示例以及 API 说明等内容。可以根据需要进行调整或补充。

```markdown
# CacheClient

`CacheClient` 是一个基于 Redis 的缓存客户端，支持 JSON 数据的存储和获取，同时具有内存中的 LRU 缓存机制，以提高读取性能。

## 特性

- 使用 `bb8` 库实现 Redis 连接池
- 支持 LRU 缓存，降低 Redis 访问频率
- 提供简洁的 API，用于缓存 JSON 数据
- 支持设置带过期时间的键值对
- 处理 Redis 数据库索引的选择

## 依赖

在你的 `Cargo.toml` 文件中添加以下依赖：

```toml
[dependencies]
anyhow = "1.0"
bb8 = "0.7"
bb8-redis = "0.7"
lru = "0.7"
redis = "0.25"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1", features = ["full"] }
```

## 使用示例

下面是如何使用 `CacheClient` 的简单示例：

```rust
use your_crate::CacheClient;
use serde::{Serialize, Deserialize};
use tokio;

#[derive(Serialize, Deserialize, Debug)]
struct MyData {
    name: String,
    value: i32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 参数说明： url, 数据库id,缓存limit，连接数
    let cache_client = CacheClient::new("redis://127.0.0.1/", Some(0), 100, 10).await?;

    // 设置 JSON 数据
    let data = MyData { name: "example".to_string(), value: 42 };
    cache_client.set_json("my_data", &data).await?;

    // 获取 JSON 数据
    if let Some(retrieved_data): Option<MyData> = cache_client.get_json("my_data").await? {
        println!("Retrieved data: {:?}", retrieved_data);
    }

    // 删除数据
    cache_client.delete("my_data").await?;

    Ok(())
}
```

## API 文档

### `CacheClient`

#### `new(url: &str, db_index: Option<u8>, cache_capacity: usize, connection_size: u32) -> Result<CacheClient>`

- 创建一个新的 `CacheClient` 实例。
- **参数**:
  - `url`: Redis 服务器的 URL。
  - `db_index`: 可选的数据库索引，默认为 `0`。
  - `cache_capacity`: LRU 缓存的最大容量。
  - `connection_size`: Redis 连接池的最大连接数。

#### `set_json<T: Serialize>(&self, key: &str, value: &T) -> Result<()>`

- 将 JSON 数据写入缓存，并同时写入 Redis。

#### `get_json<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>>`

- 从缓存或 Redis 中获取 JSON 数据。

#### `delete(&self, key: &str) -> Result<()>`

- 删除指定的键，内存和 Redis 中均会被删除。

#### `exists(&self, key: &str) -> Result<bool>`

- 检查指定的键是否存在。

#### `set_ex(&self, key: &str, value: &str, sec: u64) -> Result<()>`

- 将一个键值对写入 Redis，带过期时间。

#### `set(&self, key: &str, value: &str) -> Result<()>`

- 将一个键值对写入 Redis，不带过期时间。

#### `keys(&self, pattern: &str) -> Result<Vec<String>>`

- 获取符合模式的所有键。

## 注意事项

- 确保 Redis 服务器在指定的 URL 上运行。
- 该库在高并发环境中表现良好，适合用于缓存频繁访问的数据。

## 贡献

欢迎对 `CacheClient` 进行贡献！请查看 [CONTRIBUTING.md](./CONTRIBUTING.md) 文件以获取更多信息。

## 许可证

该项目采用 MIT 许可证，详细信息请查看 [LICENSE](./LICENSE) 文件。
```

### 注意事项

1. **代码示例**：确保在 `使用示例` 部分中用你的实际 crate 名称替换 `your_crate`。
2. **API 文档**：根据实际 API 更新文档。
3. **依赖项**：检查并确保依赖项的版本是最新的，并且适合你的项目。
4. **许可证**：如果有其他许可证要求，可以相应地更新许可证部分。

