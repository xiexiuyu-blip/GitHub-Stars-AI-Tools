#!/usr/bin/env node
// ⛔ 旧脚本（Grok 2026-10-02 写的）：会读 dev 钥匙串里的 AI 密钥并真的调用模型，花 Fox 的额度。
// 任何 AI 开发者都不得运行。要运行，必须 Fox 本人点头，并在命令前加 FSL_ALLOW_SPEND=1。
// App 内的理解卡流水线做好后删除本文件。
if (process.env.FSL_ALLOW_SPEND !== '1') {
  console.error('拒绝：这个脚本会花 AI 额度，需要 Fox 点头后设置 FSL_ALLOW_SPEND=1');
  process.exit(3);
}
// 只写 dev 库。缺 README 的不调用模型。已确认的卡不覆盖。密钥不打印。
import { DatabaseSync } from 'node:sqlite';
import { execFileSync } from 'node:child_process';

const dbPath = process.argv[2];
const limit = Number(process.argv[3] || 150);
if (!dbPath || !dbPath.includes('.dev')) {
  console.error('拒绝：数据库路径必须是 dev 库');
  process.exit(2);
}
const key = execFileSync('security', ['find-generic-password', '-s', 'fox-stars-lab-dev', '-a', 'ai-api-key:openai', '-w'], { encoding: 'utf8' }).trim();
if (!key) {
  console.error('dev Keychain 里没有 OpenAI 兼容密钥');
  process.exit(2);
}

const db = new DatabaseSync(dbPath);
const budget = 200_000;
let used = 0;
const rows = db.prepare(`
  SELECT repositories.id, repositories.full_name, substr(repo_readmes.raw_markdown, 1, 1800) AS readme
  FROM repositories
  JOIN repo_readmes ON repo_readmes.repo_id = repositories.id
  LEFT JOIN repo_ai_documents ON repo_ai_documents.repo_id = repositories.id
  WHERE repositories.sync_status = 'active'
    AND (repo_ai_documents.repo_id IS NULL OR IFNULL(repo_ai_documents.card_json, '') = '')
    AND IFNULL(repo_ai_documents.card_json, '') NOT LIKE '%"review_status":"confirmed"%'
  LIMIT ?
`).all(limit);

let wrote = 0;
let failed = 0;
for (const row of rows) {
  if (used >= budget) break;
  const prompt = `只根据下面 README 摘录写理解卡 JSON，不要编造摘录里没有的安装步骤。字段：one_liner_zh, what_it_is, form, primary_category, capabilities, problems_solved, how_to_try, maturity, zh_friendly, confidence。form 只能是 app/cli/skill/mcp/api/library/service/collection/other。\n仓库：${row.full_name}\n${row.readme}`;
  try {
    const response = await fetch('https://api.longcat.chat/openai/v1/chat/completions', {
      method: 'POST',
      headers: { Authorization: `Bearer ${key}`, 'Content-Type': 'application/json' },
      body: JSON.stringify({
        model: 'LongCat-2.0',
        temperature: 0.2,
        messages: [{ role: 'user', content: prompt }],
      }),
    });
    const payload = await response.json();
    if (!response.ok) {
      failed += 1;
      continue;
    }
    used += Number(payload.usage?.total_tokens || Math.ceil(prompt.length / 4) + 200);
    const text = payload.choices?.[0]?.message?.content || '';
    const card = JSON.parse(text.replace(/^```json\s*/i, '').replace(/```$/, '').trim());
    if (!card.one_liner_zh || !card.what_it_is) {
      failed += 1;
      continue;
    }
    const summary = String(card.what_it_is).slice(0, 400);
    const oneLiner = String(card.one_liner_zh).slice(0, 60);
    db.prepare(`
      INSERT INTO repo_ai_documents(repo_id, summary_zh, readme_zh, keywords_json, suggested_tags_json, model, prompt_version, source_hash, input_tokens, output_tokens, generated_at, one_liner_zh, card_json, card_schema_version)
      VALUES (?, ?, '', '[]', '[]', 'LongCat-2.0', 'card-v1', ?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ','now'), ?, ?, 1)
      ON CONFLICT(repo_id) DO UPDATE SET
        summary_zh = excluded.summary_zh,
        one_liner_zh = excluded.one_liner_zh,
        card_json = excluded.card_json,
        card_schema_version = 1,
        model = excluded.model,
        generated_at = excluded.generated_at
      WHERE IFNULL(repo_ai_documents.card_json, '') NOT LIKE '%"review_status":"confirmed"%'
    `).run(row.id, summary, `card-${row.id}`, Number(payload.usage?.prompt_tokens || 0), Number(payload.usage?.completion_tokens || 0), oneLiner, JSON.stringify({ ...card, review_status: 'suggested' }));
    wrote += 1;
    if (wrote % 10 === 0) console.log(`wrote=${wrote} failed=${failed} tokens=${used}`);
  } catch {
    failed += 1;
  }
}
console.log(`wrote=${wrote} failed=${failed} tokens=${used} budget=${budget}`);
