use rusqlite::{Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub struct Migration {
    pub version: &'static str,
    pub name: &'static str,
    pub sql: &'static str,
}

#[derive(Debug)]
pub struct MigrationReport {
    pub applied: Vec<String>,
    pub backup_path: Option<PathBuf>,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: "001",
        name: "initial_schema",
        sql: include_str!("../../../../../packages/storage/migrations/fox/001_initial_schema.sql"),
    },
    Migration {
        version: "002",
        name: "upstream_v120",
        sql: include_str!("../../../../../packages/storage/migrations/fox/002_upstream_v120.sql"),
    },
    Migration {
        version: "003",
        name: "ai_task_center",
        sql: include_str!("../../../../../packages/storage/migrations/fox/003_ai_task_center.sql"),
    },
    Migration {
        version: "004",
        name: "readme_chunk_citations",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/004_readme_chunk_citations.sql"
        ),
    },
    Migration {
        version: "005",
        name: "inbox_items",
        sql: include_str!("../../../../../packages/storage/migrations/fox/005_inbox_items.sql"),
    },
    Migration {
        version: "006",
        name: "inbox_documents",
        sql: include_str!("../../../../../packages/storage/migrations/fox/006_inbox_documents.sql"),
    },
    Migration {
        version: "007",
        name: "inbox_document_write_gate",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/007_inbox_document_write_gate.sql"
        ),
    },
    Migration {
        version: "008",
        name: "inbox_document_cleanup",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/008_inbox_document_cleanup.sql"
        ),
    },
    Migration {
        version: "009",
        name: "inbox_local_documents",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/009_inbox_local_documents.sql"
        ),
    },
    Migration {
        version: "010",
        name: "inbox_local_pdf_documents",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/010_inbox_local_pdf_documents.sql"
        ),
    },
    Migration {
        version: "011",
        name: "tools_and_resources",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/011_tools_and_resources.sql"
        ),
    },
    Migration {
        version: "012",
        name: "tool_ascii_name_normalization",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/012_tool_ascii_name_normalization.sql"
        ),
    },
    Migration {
        version: "013",
        name: "tool_registry_imports",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/013_tool_registry_imports.sql"
        ),
    },
    Migration {
        version: "014",
        name: "local_capability_imports",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/014_local_capability_imports.sql"
        ),
    },
    Migration {
        version: "015",
        name: "scenario_evaluation_groups",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/015_scenario_evaluation_groups.sql"
        ),
    },
    Migration {
        version: "016",
        name: "scenario_comparison_matrix",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/016_scenario_comparison_matrix.sql"
        ),
    },
    Migration {
        version: "017",
        name: "inbox_ai_understandings",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/017_inbox_ai_understandings.sql"
        ),
    },
    Migration {
        version: "018",
        name: "scenario_decisions",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/018_scenario_decisions.sql"
        ),
    },
    Migration {
        version: "019",
        name: "upstream_embedding_invalidation",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/019_upstream_embedding_invalidation.sql"
        ),
    },
    Migration {
        version: "020",
        name: "upstream_embedding_dirty_queue",
        sql: include_str!(
            "../../../../../packages/storage/migrations/fox/020_upstream_embedding_dirty_queue.sql"
        ),
    },
    Migration {
        version: "021",
        name: "capture_sources",
        sql: include_str!("../../../../../packages/storage/migrations/fox/021_capture_sources.sql"),
    },
    Migration {
        version: "022",
        name: "taxonomy",
        sql: include_str!("../../../../../packages/storage/migrations/fox/022_taxonomy.sql"),
    },
    Migration {
        version: "023",
        name: "knowledge_graph",
        sql: include_str!("../../../../../packages/storage/migrations/fox/023_knowledge_graph.sql"),
    },
    Migration {
        version: "024",
        name: "ai_cards",
        sql: include_str!("../../../../../packages/storage/migrations/fox/024_ai_cards.sql"),
    },
    Migration {
        version: "025",
        name: "ai_jobs",
        sql: include_str!("../../../../../packages/storage/migrations/fox/025_ai_jobs.sql"),
    },
    Migration {
        version: "026",
        name: "rankings",
        sql: include_str!("../../../../../packages/storage/migrations/fox/026_rankings.sql"),
    },
    Migration {
        version: "027",
        name: "reference_packs",
        sql: include_str!("../../../../../packages/storage/migrations/fox/027_reference_packs.sql"),
    },
    Migration {
        version: "028",
        name: "extensibility",
        sql: include_str!("../../../../../packages/storage/migrations/fox/028_extensibility.sql"),
    },
    Migration {
        version: "029",
        name: "public_views",
        sql: include_str!("../../../../../packages/storage/migrations/fox/029_public_views.sql"),
    },
    Migration {
        version: "030",
        name: "scenarios",
        sql: include_str!("../../../../../packages/storage/migrations/fox/030_scenarios.sql"),
    },
];

pub fn latest_migration() -> &'static Migration {
    MIGRATIONS.last().expect("迁移清单不能为空")
}

pub fn migrate_database(database_path: &Path) -> Result<MigrationReport, String> {
    if let Some(parent) = database_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("数据库目录创建失败：{}：{error}", parent.display()))?;
    }

    let mut connection = Connection::open(database_path)
        .map_err(|error| format!("SQLite 数据库打开失败：{error}"))?;
    connection
        .busy_timeout(std::time::Duration::from_millis(5000))
        .map_err(|error| format!("SQLite busy_timeout 设置失败：{error}"))?;
    connection
        .execute_batch(
            "
PRAGMA foreign_keys = ON;
PRAGMA journal_mode = WAL;
PRAGMA busy_timeout = 5000;
",
        )
        .map_err(|error| format!("SQLite 启动参数设置失败：{error}"))?;

    let applied = read_applied(&connection)?;
    refuse_name_mismatch(&applied)?;
    let pending: Vec<&Migration> = MIGRATIONS
        .iter()
        .filter(|migration| {
            !applied
                .iter()
                .any(|(version, _)| version == migration.version)
        })
        .collect();

    if pending.is_empty() {
        return Ok(MigrationReport {
            applied: Vec::new(),
            backup_path: None,
        });
    }

    integrity_check(&connection, "迁移前")?;
    let backup_path = if applied.is_empty() {
        None
    } else {
        Some(backup_database(&connection, database_path)?)
    };

    let mut applied_now = Vec::new();
    for migration in pending {
        apply_one(&mut connection, migration)?;
        applied_now.push(format!("{} {}", migration.version, migration.name));
    }
    integrity_check(&connection, "迁移后")?;

    Ok(MigrationReport {
        applied: applied_now,
        backup_path,
    })
}

fn read_applied(connection: &Connection) -> Result<Vec<(String, String)>, String> {
    let exists = connection
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .map_err(|error| format!("SQLite 迁移表检查失败：{error}"))?;
    if exists == 0 {
        return Ok(Vec::new());
    }

    let mut statement = connection
        .prepare("SELECT version, name FROM schema_migrations ORDER BY version")
        .map_err(|error| format!("SQLite 迁移记录读取失败：{error}"))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|error| format!("SQLite 迁移记录遍历失败：{error}"))?;
    let mut applied = Vec::new();
    for row in rows {
        applied.push(row.map_err(|error| format!("SQLite 迁移记录解析失败：{error}"))?);
    }
    Ok(applied)
}

fn refuse_name_mismatch(applied: &[(String, String)]) -> Result<(), String> {
    for (version, name) in applied {
        if let Some(expected) = MIGRATIONS
            .iter()
            .find(|migration| migration.version == version)
        {
            if expected.name != name {
                return Err(format!(
                    "迁移 {version} 的名称是 {name}，期望 {}。已拒绝启动，避免编号撞车被静默跳过。",
                    expected.name
                ));
            }
        }
    }
    Ok(())
}

fn apply_one(connection: &mut Connection, migration: &Migration) -> Result<(), String> {
    let sql = strip_session_pragmas(migration.sql);
    let tx = connection
        .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
        .map_err(|error| format!("迁移 {} 无法开始事务：{error}", migration.version))?;
    tx.execute_batch(&sql).map_err(|error| {
        format!(
            "迁移 {} {} 执行失败：{error}",
            migration.version, migration.name
        )
    })?;
    let recorded = tx
        .query_row(
            "SELECT name FROM schema_migrations WHERE version = ?1",
            [migration.version],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|error| format!("迁移 {} 版本记录读取失败：{error}", migration.version))?;
    match recorded {
        Some(name) if name == migration.name => {}
        Some(name) => {
            return Err(format!(
                "迁移 {} 写入的名称是 {name}，期望 {}",
                migration.version, migration.name
            ));
        }
        None => {
            return Err(format!(
                "迁移 {} {} 没有写入 schema_migrations",
                migration.version, migration.name
            ));
        }
    }
    tx.commit()
        .map_err(|error| format!("迁移 {} 提交失败：{error}", migration.version))?;
    Ok(())
}

fn strip_session_pragmas(sql: &str) -> String {
    sql.lines()
        .filter(|line| {
            let trimmed = line.trim().trim_end_matches(';').trim();
            !trimmed.eq_ignore_ascii_case("PRAGMA foreign_keys = ON")
                && !trimmed.eq_ignore_ascii_case("PRAGMA foreign_keys=ON")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn integrity_check(connection: &Connection, stage: &str) -> Result<(), String> {
    let result = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
        .map_err(|error| format!("{stage} integrity_check 失败：{error}"))?;
    if result != "ok" {
        return Err(format!("{stage} integrity_check 不是 ok：{result}"));
    }
    Ok(())
}

fn backup_database(connection: &Connection, database_path: &Path) -> Result<PathBuf, String> {
    let parent = database_path
        .parent()
        .ok_or_else(|| "数据库路径没有父目录，无法备份".to_owned())?;
    let backup_dir = parent.join("backups");
    fs::create_dir_all(&backup_dir).map_err(|error| format!("迁移备份目录创建失败：{error}"))?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("备份时间获取失败：{error}"))?
        .as_millis();
    let file_name = database_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("fox-stars-lab.sqlite3");
    let backup_path = backup_dir.join(format!("{file_name}.pre-migration-{stamp}.sqlite3"));
    let escaped = backup_path.to_string_lossy().replace('\'', "''");
    connection
        .execute_batch(&format!("VACUUM INTO '{escaped}'"))
        .map_err(|error| format!("迁移前备份失败：{error}"))?;
    let backup =
        Connection::open(&backup_path).map_err(|error| format!("迁移备份打开失败：{error}"))?;
    integrity_check(&backup, "迁移备份")?;
    Ok(backup_path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_db(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("系统时间应可用")
            .as_nanos();
        std::env::temp_dir().join(format!("fsl-{label}-{nanos}.sqlite3"))
    }

    fn count_query(path: &Path, sql: &str) -> i64 {
        let connection = Connection::open(path).expect("应能打开测试库");
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("计数查询应成功")
    }

    #[test]
    fn empty_database_applies_every_migration() {
        let path = temp_db("empty");
        let report = migrate_database(&path).expect("空库应能迁移到 020");
        assert_eq!(report.applied.len(), MIGRATIONS.len());
        assert!(report.backup_path.is_none());

        let connection = Connection::open(&path).expect("应能打开空库结果");
        let versions = read_applied(&connection).expect("应能读取版本");
        assert_eq!(versions.len(), MIGRATIONS.len());
        for migration in MIGRATIONS {
            assert!(
                versions
                    .iter()
                    .any(|(version, name)| version == migration.version && name == migration.name),
                "缺少 {} {}",
                migration.version,
                migration.name
            );
        }
        assert_eq!(
            count_query(
                &path,
                "SELECT COUNT(*) FROM sqlite_master WHERE name = 'embedding_dirty_queue'"
            ),
            1
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn wrong_migration_name_refuses_startup() {
        let path = temp_db("mismatch");
        migrate_database(&path).expect("先建立 001–020");
        let connection = Connection::open(&path).expect("应能打开库");
        connection
            .execute(
                "UPDATE schema_migrations SET name = 'wrong' WHERE version = '019'",
                [],
            )
            .expect("应能改错名称");
        drop(connection);

        let error = migrate_database(&path).expect_err("名称不符必须拒绝启动");
        assert!(error.contains("019"), "{error}");
        assert!(error.contains("拒绝启动"), "{error}");
        let _ = fs::remove_file(&path);
    }

    /// 造一个「正好停在 018」的库，并往几张表里放真实形状的数据，其中包括
    /// 「应用已退出，任务中断」的旧失败项（真实库里有 139 个）。
    /// 不再读本机的 dev 库：它已经迁到 029，拿它当 018 夹具，这个测试永远过不了。
    fn build_018_fixture(path: &Path) {
        let mut connection = Connection::open(path).expect("应能创建 018 夹具");
        connection
            .execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")
            .expect("夹具参数应可设置");
        for migration in MIGRATIONS
            .iter()
            .filter(|migration| migration.version <= "018")
        {
            apply_one(&mut connection, migration).expect("夹具迁移应成功");
        }
        connection
            .execute_batch(
                "
INSERT INTO github_accounts(id, login, token_ref) VALUES ('acct', 'fox', 'test');
INSERT INTO repositories(id, account_id, owner, name, full_name, description, language, topics_json, html_url, sync_status, starred_at) VALUES
  ('r1', 'acct', 'a', 'one', 'a/one', '第一个', 'Python', '[\"mcp\"]', 'https://github.com/a/one', 'active', '2026-09-01T00:00:00Z'),
  ('r2', 'acct', 'b', 'two', 'b/two', '第二个', 'TypeScript', '[]', 'https://github.com/b/two', 'active', '2026-09-02T00:00:00Z'),
  ('r3', 'acct', 'c', 'three', 'c/three', '已取消 Star', NULL, '[]', 'https://github.com/c/three', 'removed', '2026-09-03T00:00:00Z');
INSERT INTO repo_ai_documents(repo_id, summary_zh, readme_zh, keywords_json, suggested_tags_json, model, prompt_version, source_hash, generated_at) VALUES
  ('r1', '这是第一个仓库的摘要', '', '[]', '[]', 'test', 'v1', 'hash', '2026-09-01T00:00:00Z');
INSERT INTO inbox_items(id, account_id, original_url, normalized_url, item_type) VALUES
  ('i1', 'acct', 'https://example.com/a', 'https://example.com/a', 'webpage');
INSERT INTO ai_task_runs(id, account_id, task_type, status) VALUES ('run1', 'acct', 'batch_generate_ai_documents', 'failed');
INSERT INTO ai_task_items(id, run_id, repository_id, full_name, status, reason) VALUES
  ('t1', 'run1', 'r1', 'a/one', 'failed', '应用已退出，任务中断，可重试。'),
  ('t2', 'run1', 'r2', 'b/two', 'failed', '模型输出不是可解析的 JSON');
",
            )
            .expect("夹具数据应能写入");
    }

    fn table_counts(path: &Path) -> std::collections::BTreeMap<String, i64> {
        let connection = Connection::open(path).expect("应能打开测试库");
        let names: Vec<String> = {
            let mut statement = connection
                .prepare(
                    "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
                )
                .expect("表清单应可读取");
            statement
                .query_map([], |row| row.get(0))
                .expect("表清单应可遍历")
                .map(|name| name.expect("表名应可读取"))
                .collect()
        };
        names
            .into_iter()
            .map(|name| {
                let count: i64 = connection
                    .query_row(&format!("SELECT COUNT(*) FROM \"{name}\""), [], |row| {
                        row.get(0)
                    })
                    .expect("计数应成功");
                (name, count)
            })
            .collect()
    }

    #[test]
    fn synthetic_018_database_only_appends_newer_migrations_and_keeps_every_row() {
        let path = temp_db("fixture-018");
        build_018_fixture(&path);
        let before = table_counts(&path);
        assert_eq!(before["repositories"], 3);
        assert_eq!(before["ai_task_items"], 2);

        let report = migrate_database(&path).expect("018 库应只追加更新的迁移");
        let expected: Vec<String> = MIGRATIONS
            .iter()
            .filter(|migration| migration.version > "018")
            .map(|migration| format!("{} {}", migration.version, migration.name))
            .collect();
        assert_eq!(report.applied, expected);
        assert!(report.backup_path.is_some(), "有数据的库迁移前必须先备份");

        let after = table_counts(&path);
        for (table, count) in &before {
            // 版本记录表本来就会随迁移增加，下面单独核对
            if table == "schema_migrations" {
                continue;
            }
            assert_eq!(
                after.get(table),
                Some(count),
                "迁移不能改动已有表 {table} 的行数"
            );
        }
        assert_eq!(
            after["schema_migrations"],
            before["schema_migrations"] + report.applied.len() as i64
        );
        // 025 把旧失败项按仓库转成待重试任务：中断的、JSON 出错的各一条
        assert_eq!(after["ai_jobs"], 2);
        assert_eq!(
            count_query(
                &path,
                "SELECT COUNT(*) FROM ai_jobs WHERE error_kind = 'interrupted' AND status = 'queued'"
            ),
            1
        );
        assert_eq!(
            count_query(
                &path,
                "SELECT COUNT(*) FROM ai_jobs WHERE error_kind = 'bad_json'"
            ),
            1
        );

        // 再启动一次：没有新迁移，不应再生成备份
        let again = migrate_database(&path).expect("第二次启动应直接通过");
        assert!(again.applied.is_empty() && again.backup_path.is_none());

        let _ = fs::remove_file(&path);
        if let Some(backup) = report.backup_path {
            let _ = fs::remove_file(backup);
        }
    }

    #[test]
    #[ignore = "会写入 dev 库并生成迁移备份，只在 FOX_STARS_MIGRATE_DEV=1 时运行"]
    fn migrate_live_dev_database_when_explicitly_requested() {
        if std::env::var("FOX_STARS_MIGRATE_DEV").ok().as_deref() != Some("1") {
            panic!("拒绝迁移：未设置 FOX_STARS_MIGRATE_DEV=1");
        }
        let path = PathBuf::from(
            "/Users/fox/Library/Application Support/com.foxwork.fox-stars-lab.dev/fox-stars-lab.sqlite3",
        );
        assert!(path.to_string_lossy().contains(".dev"));
        assert!(!path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "com.foxwork.fox-stars-lab"));
        let before = (
            count_query(
                &path,
                "SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'",
            ),
            count_query(&path, "SELECT COUNT(*) FROM repo_readmes"),
            count_query(&path, "SELECT COUNT(*) FROM repo_ai_documents"),
            count_query(&path, "SELECT COUNT(*) FROM repo_ai_citations"),
            count_query(&path, "SELECT COUNT(*) FROM inbox_items"),
            count_query(&path, "SELECT COUNT(*) FROM tools"),
        );
        let report = migrate_database(&path).expect("dev 库应能追加迁移");
        let after = (
            count_query(
                &path,
                "SELECT COUNT(*) FROM repositories WHERE sync_status = 'active'",
            ),
            count_query(&path, "SELECT COUNT(*) FROM repo_readmes"),
            count_query(&path, "SELECT COUNT(*) FROM repo_ai_documents"),
            count_query(&path, "SELECT COUNT(*) FROM repo_ai_citations"),
            count_query(&path, "SELECT COUNT(*) FROM inbox_items"),
            count_query(&path, "SELECT COUNT(*) FROM tools"),
        );
        assert_eq!(before, after);
        assert!(report
            .backup_path
            .as_ref()
            .is_some_and(|backup| backup.exists()));
        let version = count_query(
            &path,
            "SELECT MAX(CAST(version AS INTEGER)) FROM schema_migrations",
        );
        assert_eq!(version, 29);
    }

    #[test]
    #[ignore = "会写 dev 库的分类、建议卡和榜单，只在 FOX_STARS_REFRESH_DEV=1 时运行"]
    fn refresh_live_dev_knowledge_and_rankings() {
        if std::env::var("FOX_STARS_REFRESH_DEV").ok().as_deref() != Some("1") {
            panic!("拒绝：未设置 FOX_STARS_REFRESH_DEV=1");
        }
        let path = PathBuf::from(
            "/Users/fox/Library/Application Support/com.foxwork.fox-stars-lab.dev/fox-stars-lab.sqlite3",
        );
        assert!(path.to_string_lossy().contains(".dev"));
        let seeded = crate::v2_ops::seed_taxonomy(&path).expect("分类种子");
        let classified = crate::v2_ops::rule_classify(&path).expect("规则分类");
        let cards = crate::v2_ops::import_summary_cards(&path).expect("旧摘要转卡");
        let queued = crate::v2_ops::enqueue_missing_cards(&path).expect("缺卡入队");
        crate::v2_ops::set_daily_budget(&path, 200_000).expect("预算");
        let rankings = crate::rankings_live::refresh_default_sources(&path).expect("榜单刷新");
        eprintln!("seeded={seeded} classified={classified} cards={cards} queued={queued} rankings={rankings}");
        assert!(!crate::v2_ops::source_enabled(&path, "github_trending_html").expect("trending"));
    }
}
