#!/usr/bin/env node
// Fox Stars Lab 本地资料库查询。只读，除非显式 --write-usage。
import { DatabaseSync } from 'node:sqlite';
import { pathToFileURL } from 'node:url';

const args = process.argv.slice(2);
const command = args[0];
const dbPath = valueOf('--db') || process.env.FOX_STARS_LAB_DB;

if (!command || !dbPath || command === '--help') {
  process.stdout.write(`用法：
  node tools/fsl.mjs search 关键词 --db <sqlite>
  node tools/fsl.mjs health --db <sqlite>
  node tools/fsl.mjs log-usage <仓库id> useful|meh|useless --note 备注 --db <sqlite> --write-usage

默认只读。写回使用记录必须同时带 --write-usage。不会改仓库表。
`);
  process.exit(command === '--help' ? 0 : 1);
}

const writeUsage = args.includes('--write-usage');
const db = new DatabaseSync(dbPath, { readOnly: !writeUsage, timeout: 1500 });
if (!writeUsage) db.exec('PRAGMA query_only=ON');

if (command === 'search') {
  const query = args[1] || '';
  const pattern = `%${query.replace(/[%_\\]/g, (char) => `\\${char}`)}%`;
  const rows = db.prepare(`
    SELECT repositories.full_name, IFNULL(repo_ai_documents.summary_zh, IFNULL(repositories.description, '')) AS summary
    FROM repositories
    LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
    WHERE repositories.sync_status = 'active'
      AND (? = '' OR repositories.full_name LIKE ? ESCAPE '\\' OR IFNULL(repositories.description,'') LIKE ? ESCAPE '\\' OR IFNULL(repo_ai_documents.summary_zh,'') LIKE ? ESCAPE '\\')
    ORDER BY repositories.starred_at DESC
    LIMIT 20
  `).all(query, pattern, pattern, pattern);
  for (const row of rows) {
    process.stdout.write(`${row.full_name}\t${String(row.summary ?? '').slice(0, 120)}\n`);
  }
} else if (command === 'health') {
  const integrity = db.prepare('PRAGMA integrity_check').get();
  const version = db.prepare('SELECT COALESCE(MAX(version), \'\') AS version FROM schema_migrations').get();
  process.stdout.write(`完整性 ${integrity.integrity_check}，迁移版本 ${version.version}\n`);
} else if (command === 'log-usage') {
  if (!writeUsage) {
    process.stderr.write('写回使用记录必须加 --write-usage\n');
    process.exit(2);
  }
  const entityId = args[1];
  const verdict = args[2];
  const note = valueOf('--note') || '';
  if (!['useful', 'meh', 'useless'].includes(verdict)) {
    process.stderr.write('反馈只能是 useful、meh 或 useless\n');
    process.exit(2);
  }
  const account = db.prepare('SELECT id FROM github_accounts LIMIT 1').get();
  if (!account) {
    process.stderr.write('库里还没有 GitHub 账号\n');
    process.exit(2);
  }
  db.prepare(`INSERT INTO usage_log(id, account_id, entity_type, entity_id, verdict, note, via) VALUES (?, ?, 'repository', ?, ?, ?, 'mcp')`).run(
    `cli-${Date.now()}`,
    account.id,
    entityId,
    verdict,
    note.slice(0, 4000),
  );
  process.stdout.write('已写入使用记录\n');
} else {
  process.stderr.write(`不认识的命令：${command}\n`);
  process.exit(1);
}

function valueOf(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : '';
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  // already executed above
}
