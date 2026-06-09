# Data Layout

本项目 reader-core 使用以下数据目录布局（相对于 app data 目录）。

## 目录结构

```text
reader/
  reader.db                          # SQLite 数据库（书源、配置、用户数据）
  sources/
    script-js/                       # JavaScript 书源（.js 文件）
    legado-json/                     # Legado JSON 书源（.legado.json 文件）
  cache/
    chapters/                        # 章节正文缓存（按书籍 ID 分目录）
    audio/                           # 音频缓存（TTS、有声书）
  config/                            # 键值配置（JSON 文档）
  data/
    local/
      shelf.json                     # 书架数据（书籍列表 + 元数据）
      shelf/                         # 单本书籍数据
        {safe_book_id}/
          chapters.json              # 已缓存章节列表
          content/                   # 章节正文
            {chapter_index}.txt      # 单章正文
          episode-progress.json      # 音频/视频播放进度
  drafts/                            # 书源草稿（AI 编辑器用，不出现于已安装列表）
```

## 数据分类

| 路径                   | 是否可删除 | 是否需备份 | 含敏感信息             | 迁移策略       |
| ---------------------- | ---------- | ---------- | ---------------------- | -------------- |
| reader.db              | 否         | 是         | 是（cookie、登录信息） | 增量 migration |
| sources/script-js/     | 否         | 是         | 否                     | 文件复制       |
| sources/legado-json/   | 否         | 是         | 否                     | 文件复制       |
| cache/chapters/        | 是         | 否         | 否                     | 无需迁移       |
| cache/audio/           | 是         | 否         | 否                     | 无需迁移       |
| config/                | 否         | 是         | 可能                   | JSON 导出      |
| data/local/shelf.json  | 否         | 是         | 否                     | JSON 导出      |
| data/local/shelf/{id}/ | 否         | 是         | 否                     | JSON 导出      |
| drafts/                | 是         | 否         | 否                     | 无需迁移       |

## 隐私与安全

- 登录信息（cookie、token）存储在 `reader.db` 中，不导出到普通备份
- 导出功能只导出用户明确选择的书架数据
- 缓存目录可随时清理，不影响书架和阅读进度
- 书源文件存储在 sources/ 下，删除需通过应用内管理界面
