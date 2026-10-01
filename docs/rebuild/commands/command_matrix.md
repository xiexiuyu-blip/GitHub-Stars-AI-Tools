# Tauri 命令矩阵（Fox 1.4.0-fox.1 vs fork b496428 vs upstream 1.5.3 HEAD a4a8ff1）

- Fox 安装版二进制命令：104 个；fork b496428：46 个；upstream：62 个
- **Fox 独有（必须重建）：52 个**；upstream 新增而 Fox 1.4.0 没有：12 个（合并 upstream 即获得）

来源：Fox 列表来自 `/Applications/Fox Stars Lab.app/Contents/MacOS/gsat-desktop` 字符串；fork/upstream 来自 `apps/desktop/src-tauri/src/lib.rs` 的 `generate_handler![]`。

| 分类 | 命令 | fork b496428 | upstream 1.5.3 | Fox 1.4.0 | 结论 |
|---|---|---|---|---|---|
| AI 任务中心 | `get_ai_task_run` |  |  | ✓ | **Fox 独有→重建** |
| AI 任务中心 | `list_ai_task_runs` |  |  | ✓ | **Fox 独有→重建** |
| AI 任务中心 | `retry_ai_task_items` |  |  | ✓ | **Fox 独有→重建** |
| AI 助手配置(MCP) | `cancel_ai_assistant_preview` |  |  | ✓ | **Fox 独有→重建** |
| AI 助手配置(MCP) | `execute_ai_assistant_config` |  |  | ✓ | **Fox 独有→重建** |
| AI 助手配置(MCP) | `get_ai_assistant_status` |  |  | ✓ | **Fox 独有→重建** |
| AI 助手配置(MCP) | `preview_ai_assistant_config` |  |  | ✓ | **Fox 独有→重建** |
| 健康/运行时 | `check_runtime_readiness` | ✓ | ✓ | ✓ | 共有 |
| 健康/运行时 | `diagnose_ai_request` | ✓ |  | ✓ | fork 已有(upstream 无)→从 fork cherry-pick |
| 健康/运行时 | `get_app_identity` | ✓ |  | ✓ | fork 已有(upstream 无)→从 fork cherry-pick |
| 健康/运行时 | `get_health_overview` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `add_scenario_candidate` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `add_standard_scenario_criteria` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `archive_scenario` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `confirm_scenario_decision` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `delete_scenario` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `delete_scenario_criterion` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `get_scenario_comparison` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `get_scenario_decision_history` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `get_scenario_detail` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `list_scenarios` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `move_scenario_candidate` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `move_scenario_criterion` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `remove_scenario_candidate` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `save_scenario` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `save_scenario_comparison_cell` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `save_scenario_criterion` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `save_scenario_decision_draft` |  |  | ✓ | **Fox 独有→重建** |
| 场景/矩阵/决策 | `update_scenario_candidate` |  |  | ✓ | **Fox 独有→重建** |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `batch_generate_repository_ai_documents` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `clear_ai_api_key` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `clear_app_settings` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `clear_embedding_api_key` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `clear_github_token` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `clear_local_database` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `create_tag` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `delete_local_embedding_model` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `delete_tag` | ✓ | ✓ | ✓ | 共有 |
| 工具卡 | `delete_tool` |  |  | ✓ | **Fox 独有→重建** |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `disable_embedding_runtime` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `enable_local_embedding` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `explain_ai_search_topic` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `export_annotation_gist` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `export_repository_library_gist` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `fetch_github_ranking_readme` |  | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `fetch_github_recommendation_readme` |  | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `fetch_repository_readme` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `fetch_repository_readmes` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `generate_ai_tag_network` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `generate_repository_ai_document` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_app_settings` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_backend_status` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_dashboard_stats` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_embedding_runtime_status` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_github_auth_state` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_profile_stats` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_repository_annotation` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_repository_detail` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_repository_filter_counts` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_tag_network_data` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `get_vector_index_status` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `has_ai_api_key` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `has_embedding_api_key` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `import_annotation_gist` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `import_repository_library_gist` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_ai_models` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_github_rankings` |  | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_github_recommendation_candidates` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_personal_rankings` |  | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_repositories` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_repository_languages` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `list_tags` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `open_external_url` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `rebuild_vector_index` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `recommend_github_repositories` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `retry_embedding_setup` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `save_ai_api_key` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `save_app_settings` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `save_embedding_api_key` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `save_github_token` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `save_repository_annotation` | ✓ | ✓ | ✓ | 共有 |
| 工具卡 | `save_tool` |  |  | ✓ | **Fox 独有→重建** |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `search_repositories` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `set_repository_tags` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `star_github_ranking_repository` |  | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `star_github_recommendation_candidate` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `sync_github_stars` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `test_ai_connection` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `test_embedding_connection` |  | ✓ |  | upstream 新增→合并获得 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `translate_github_recommendation_readme` |  | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `update_github_recommendation_candidate_status` | ✓ | ✓ | ✓ | 共有 |
| 基础(仓库/标签/AI/推荐/榜单/Gist/设置) | `update_tag` | ✓ | ✓ | ✓ | 共有 |
| 备份 | `create_local_backup` |  |  | ✓ | **Fox 独有→重建** |
| 备份 | `list_local_backups` |  |  | ✓ | **Fox 独有→重建** |
| 备份 | `restore_local_backup` |  |  | ✓ | **Fox 独有→重建** |
| 备份 | `show_local_backups_in_finder` |  |  | ✓ | **Fox 独有→重建** |
| 备份 | `validate_local_backup` |  |  | ✓ | **Fox 独有→重建** |
| 工具卡 | `get_tool_detail` |  |  | ✓ | **Fox 独有→重建** |
| 工具卡 | `list_tool_candidates` |  |  | ✓ | **Fox 独有→重建** |
| 工具卡 | `list_tools` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `archive_inbox_item` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `create_inbox_item` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `delete_inbox_item` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `generate_inbox_ai_understanding` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `get_inbox_ai_understanding` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `get_inbox_document` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `import_local_inbox_documents` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `list_inbox_items` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `parse_inbox_webpage` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `review_inbox_ai_understanding` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `update_inbox_item` |  |  | ✓ | **Fox 独有→重建** |
| 投喂 Inbox | `update_local_inbox_item` |  |  | ✓ | **Fox 独有→重建** |
| 注册表/本地能力导入 | `commit_local_capability` |  |  | ✓ | **Fox 独有→重建** |
| 注册表/本地能力导入 | `commit_tool_registry` |  |  | ✓ | **Fox 独有→重建** |
| 注册表/本地能力导入 | `preview_local_capability` |  |  | ✓ | **Fox 独有→重建** |
| 注册表/本地能力导入 | `preview_tool_registry` |  |  | ✓ | **Fox 独有→重建** |
