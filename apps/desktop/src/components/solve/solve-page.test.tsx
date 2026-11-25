import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it } from 'vitest';
import { ToastProvider } from '@/components/kit';
import type { SolveHit, SolveResult } from '@/lib/contracts';
import { saveSolveDraft } from '@/lib/solve-draft';
import { SolvePage } from '@/pages/v2/solve';

/** 整页渲染一次：确认新的格式列表、「交接单」叫法和复制按钮真的接在页面上。 */
function renderPage(initialQuestion?: string): string {
  return renderToStaticMarkup(
    <ToastProvider>
      <SolvePage initialQuestion={initialQuestion} />
    </ToastProvider>,
  );
}

/** 页面里「格式」那一组单选框的 HTML（里面没有嵌套的 div，第一个 </div> 就是它的结尾）。 */
function formatGroupOf(markup: string): string {
  return /<div role="radiogroup"[\s\S]*?<\/div>/.exec(markup)?.[0] ?? '';
}

function hit(id: string): SolveHit {
  return {
    id,
    fullName: `fox/${id}`,
    htmlUrl: `https://github.com/fox/${id}`,
    starsCount: 12,
    oneLiner: `${id} 的一句话`,
    form: 'skill',
    scenarioIds: [],
    matched: ['封面'],
    missing: [],
    why: '对得上的地方：「封面」→名字里有',
    score: 1,
    fullMatch: true,
    hasCard: true,
  };
}

function found(preselected: string[]): SolveResult {
  return {
    query: '做小红书封面',
    intents: ['封面'],
    extras: [],
    hits: ['a', 'b', 'c'].map(hit),
    preselected,
    fullMatchCount: 3,
    gapIntents: [],
    suggestedScenarioId: null,
    suggestedScenarioCount: null,
    pendingMatches: 0,
  };
}

/** 切走再切回来：页面从草稿里恢复上次的结果和勾选（页面里的草稿是模块级的，每个用例后清掉）。 */
function restoreDraft(result: SolveResult, picked: string[], direct = false): void {
  saveSolveDraft({ query: result.query, activeQuery: result.query, result, picked, format: 'markdown', direct });
}

afterEach(() => saveSolveDraft(null));

describe('SolvePage：整页接线', () => {
  it('生成尚未就绪时，真实复制按钮禁用', () => {
    restoreDraft(found(['a']), ['a']);
    const markup = renderPage();
    const copyButton = markup.match(/<button\b[^>]*>[\s\S]*?<\/button>/g)?.find((button) => button.includes('复制交接单给 AI 助手')) ?? '';
    expect(copyButton).toContain('disabled=""');
    expect(markup).toContain('aria-label="正在整理交接单"');
  });

  it('消费过带入问题后普通返回恢复草稿，明确再次带入同一句仍开始新任务', () => {
    const result = found(['a', 'b', 'c']);
    restoreDraft(result, ['c']);
    expect(renderPage('')).toContain('已勾 1 个');
    const newTask = renderPage(result.query);
    expect(newTask).not.toContain('已勾 1 个');
    expect(newTask).toContain('aria-label="正在找"');
  });

  it('AI 精选开关旁写明费用、额度、独立于自动整理，以及关闭不能撤回已发请求', () => {
    const markup = renderPage();
    expect(markup).toContain('aria-describedby="solve-ai-cost"');
    expect(markup).toContain('会花钱（每次约 4 千 token，独立于「自动整理」开关，受 AI 精选额度限制）');
    expect(markup).toContain('具体费用以服务商账单为准');
    expect(markup).toContain('关闭只影响后续精选，已发出的请求可能仍会计费');
  });

  it('右边面板叫「交接单」，复制按钮叫「复制交接单给 AI 助手」，没有旧叫法', () => {
    const markup = renderPage();
    expect(markup).toContain('找方案');
    expect(markup).toContain('>交接单</h3>');
    expect(markup).toContain('复制交接单给 AI 助手');
    expect(markup).toContain('再整理成给 AI 助手的交接单');
    expect(markup).not.toContain('参考包');
    expect(markup).not.toContain('没有合适的？直接交给 AI 助手');
    expect(markup).not.toContain('都不合适？直接交给 AI 助手');
  });

  it('格式是竖排的三个单选项，放在面板标题和预览之间，没有不换行的按钮', () => {
    const markup = renderPage();
    const group = formatGroupOf(markup);
    expect(group).toContain('role="radiogroup"');
    expect((group.match(/type="radio"/g) ?? [])).toHaveLength(3);
    expect(group).toContain('文档');
    expect(group).toContain('Markdown，直接粘给 AI 助手，默认');
    expect(group).toContain('长期规则');
    expect(group).toContain('AGENTS.md，存成文件放进工作文件夹');
    expect(group).toContain('程序用');
    expect(group).toContain('JSON，一般不用选');
    expect(group).not.toContain('<button');
    expect(group).not.toContain('whitespace-nowrap');
    // 页面上不再有横排的分段按钮：格式只剩这一组单选框
    expect((markup.match(/role="radiogroup"/g) ?? [])).toHaveLength(1);
    expect(markup.indexOf('>交接单</h3>')).toBeLessThan(markup.indexOf('role="radiogroup"'));
    expect(markup.indexOf('role="radiogroup"')).toBeLessThan(markup.indexOf('复制交接单给 AI 助手'));
  });

  it('还没搜的时候，列表顶部不说「已替你勾了」，也不说已勾几个', () => {
    const markup = renderPage();
    expect(markup).not.toContain('已替你勾了');
    expect(markup).not.toContain('已勾 ');
  });

  it('从草稿回来：列表顶部的「已勾 N 个」和右边的「已选 N 个」是同一个数（后端当初默认勾了 3 个也不再写它）', () => {
    restoreDraft(found(['a', 'b', 'c']), ['a', 'c']);
    const markup = renderPage();
    expect(markup).toContain('已勾 2 个');
    expect(markup).toContain('已选 2 个');
    expect(markup).not.toContain('已替你勾了');
    expect(markup).not.toContain('已勾 3 个');
    // 列表里每一行都是新的叫法
    expect(markup).toContain('加入交接单 fox/b');
    expect(markup).toContain('从交接单移除 fox/a');
    expect(markup).toContain('AI 助手会优先用它，其余是备选');
    expect(markup).not.toContain('为什么相关');
  });

  it('从草稿回来、什么都没勾：顶上说已勾 0 个，没有默认勾选时右边照旧说「没有特别对口的，所以没替你默认勾选」', () => {
    restoreDraft(found([]), []);
    const markup = renderPage();
    expect(markup).toContain('已勾 0 个');
    expect(markup).toContain('已选 0 个');
    expect(markup).toContain('没有特别对口的，所以没替你默认勾选');
    expect(markup).toContain('不用这些工具，直接交给 AI 助手');
  });

  it('列表顶上那句提示缩短了，不会在 900 宽的窗口里折成两行、留下孤零零的一个字', () => {
    restoreDraft(found(['a', 'b']), ['a', 'b']);
    const markup = renderPage();
    const hint = '勾选的写进交接单，点一行看详情';
    expect(markup).toContain(`>${hint}</span>`);
    expect(markup).not.toContain('勾选的会写进交接单');
    // 十五个字以内：之前十七个字，在 900 宽的窗口里放不下最后一个字
    expect(Array.from(hint).length).toBeLessThanOrEqual(15);
  });

  it('预览标题栏的按钮叫「看 Markdown 原文」（点了变成「看排版效果」），不再是含糊的「看原文」「看排版」', () => {
    restoreDraft(found(['a', 'b']), ['a', 'b']);
    const markup = renderPage();
    expect(markup).toContain('看 Markdown 原文');
    expect(markup).not.toContain('>看原文<');
    expect(markup).not.toContain('>看排版<');
    expect(markup).not.toContain('看排版效果');
  });

  it('从草稿回来、是直接交给 AI 助手：顶上不说勾了几个', () => {
    restoreDraft(found(['a']), ['a'], true);
    const markup = renderPage();
    expect(markup).not.toContain('已勾 ');
    expect(markup).not.toContain('已替你勾了');
    expect(markup).toContain('不选工具 · 直接交给 AI 助手');
    expect(markup).toContain('改回挑工具');
  });
});
