# 表矩阵（Fox live 018 vs fork b496428 vs upstream 1.5.3）

| 表 | 创建于 Fox 迁移 | fork | upstream | Fox live | 结论 |
|---|---|---|---|---|---|
| `annotations` | 001 | ✓ | ✓ | ✓ | 共有 |
| `github_accounts` | 001 | ✓ | ✓ | ✓ | 共有 |
| `jobs` | 001 | ✓ | ✓ | ✓ | 共有 |
| `repo_ai_documents` | 001 | ✓ | ✓ | ✓ | 共有 |
| `repo_embeddings` | 001 | ✓ | ✓ | ✓ | 共有 |
| `repo_readmes` | 001 | ✓ | ✓ | ✓ | 共有 |
| `repo_tags` | 001 | ✓ | ✓ | ✓ | 共有 |
| `repositories` | 001 | ✓ | ✓ | ✓ | 共有 |
| `schema_migrations` | 001 | ✓ | ✓ | ✓ | 共有 |
| `tags` | 001 | ✓ | ✓ | ✓ | 共有 |
| `github_ranking_cache` | 002 |  | ✓ | ✓ | 共有 |
| `github_recommendation_candidates` | 002 | ✓ | ✓ | ✓ | 共有 |
| `github_recommendation_documents` | 002 |  | ✓ | ✓ | 共有 |
| `ai_task_items` | 003 |  |  | ✓ | **Fox 独有→重建** |
| `ai_task_runs` | 003 |  |  | ✓ | **Fox 独有→重建** |
| `repo_ai_citations` | 004 |  |  | ✓ | **Fox 独有→重建** |
| `inbox_items` | 005 |  |  | ✓ | **Fox 独有→重建** |
| `inbox_documents` | 006 |  |  | ✓ | **Fox 独有→重建** |
| `inbox_local_documents` | 009 |  |  | ✓ | **Fox 独有→重建** |
| `tool_forms` | 011 |  |  | ✓ | **Fox 独有→重建** |
| `tool_inbox_items` | 011 |  |  | ✓ | **Fox 独有→重建** |
| `tool_repositories` | 011 |  |  | ✓ | **Fox 独有→重建** |
| `tools` | 011 |  |  | ✓ | **Fox 独有→重建** |
| `tool_registry_entries` | 013 |  |  | ✓ | **Fox 独有→重建** |
| `tool_registry_imports` | 013 |  |  | ✓ | **Fox 独有→重建** |
| `local_capability_entries` | 014 |  |  | ✓ | **Fox 独有→重建** |
| `local_capability_imports` | 014 |  |  | ✓ | **Fox 独有→重建** |
| `scenario_evaluation_candidates` | 015 |  |  | ✓ | **Fox 独有→重建** |
| `scenario_evaluation_groups` | 015 |  |  | ✓ | **Fox 独有→重建** |
| `scenario_comparison_cells` | 016 |  |  | ✓ | **Fox 独有→重建** |
| `scenario_comparison_criteria` | 016 |  |  | ✓ | **Fox 独有→重建** |
| `inbox_ai_task_items` | 017 |  |  | ✓ | **Fox 独有→重建** |
| `inbox_ai_understandings` | 017 |  |  | ✓ | **Fox 独有→重建** |
| `scenario_decision_evidence` | 018 |  |  | ✓ | **Fox 独有→重建** |
| `scenario_decisions` | 018 |  |  | ✓ | **Fox 独有→重建** |
| `embedding_dirty_queue` | - |  | ✓ |  | upstream 新增→迁移 019/020 引入 |

差异说明：
- `annotations.read_status` CHECK：upstream 仅 unread/read/later；Fox(=fork) 扩展为 8 值。合并后必须保留 Fox 版本，前端 reading-status.ts 同步。
- `repo_ai_documents`：Fox 004 增加 summary_mode/coverage_status/source_char_count/processed_char_count/chunk_count/citation_count 6 列，upstream 无。
- upstream 002/003 的 25 个 trigger + `embedding_dirty_queue` 表在 Fox live 库中不存在，已验证可在 Fox 018 结构上无错执行（IF NOT EXISTS），但 schema_migrations 的 002/003 记录会被 INSERT OR IGNORE 静默吞掉（名字冲突）。
