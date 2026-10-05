import { describe, expect, it } from 'vitest';
import SOURCE from '@/pages/v2/toolbox.tsx?raw';
import { SLOW_ORGANIZE_COPY } from '@/lib/automation-view';

describe('自动整理页：确认入口和纯函数接线', () => {
  const panelSource = SOURCE.slice(SOURCE.indexOf('function FullScanPanelView('), SOURCE.indexOf('export function ToolboxPage()'));

  it('首次点击只展开确认，确认开始才调用原命令并带 confirmed: true', () => {
    const askSource = SOURCE.slice(SOURCE.indexOf('function askFullScan()'), SOURCE.indexOf('function startFullScan()'));
    expect(askSource).toContain('setPendingFullScan(true)');
    expect(askSource).not.toContain('invoke');
    expect(SOURCE).toContain("invoke<AutomationStatus>('start_full_scan', { confirmed: true })");
    expect(panelSource).toContain('onClick={props.onAsk}');
    expect(panelSource).toContain('onClick={props.onStart}>确认开始</Button>');
    expect(panelSource).toContain('onClick={props.onCancel}>取消</Button>');
  });

  it('确认块出现时滚进可视区，并将焦点放到确认开始；刷新状态不反复抢焦点', () => {
    expect(panelSource).toContain('ref={confirmationRef}');
    expect(panelSource).toContain("scrollIntoView({ block: 'nearest' })");
    expect(panelSource).toContain("querySelector<HTMLButtonElement>('[data-confirm-full-scan]')?.focus({ preventScroll: true })");
    expect(panelSource).toContain('<Button data-confirm-full-scan');
    expect(panelSource).toContain('}, [showingConfirmation])');
  });

  it('面板状态、标题、说明、正文、按钮都由 scanPanel 提供，页面不另判断额度', () => {
    expect(SOURCE).toContain('const scanPanel = fullScanPanel(automation)');
    expect(SOURCE).toContain('panel={scanPanel}');
    for (const field of ['title', 'description', 'body', 'buttonLabel']) {
      expect(panelSource).toContain(`{panel.${field}}`);
    }
    expect(panelSource).not.toContain('budgetRemaining');
    expect(panelSource).not.toContain('dailyBudgetTokens');
    expect(SOURCE).not.toContain('budgetRemaining');
    expect(SOURCE).toContain('hint={AUTO_ORGANIZE_DEFINITION}');
    expect(SOURCE).toContain('const countsLine = organizeCountsLine(automation)');
    expect(SOURCE).toContain('{DAILY_ORGANIZE_TITLE}');
    expect(SOURCE).toContain('{DAILY_ORGANIZE_DESCRIPTION}');
    expect(SOURCE).toContain('{DAILY_ORGANIZE_DURING_SCAN}');
    expect(SOURCE).toContain('<Metric {...dailyBudgetMetric(automation)} />');
  });

  it('收藏分项行显示 countsLine，位于状态说明之后、整理面板之前', () => {
    expect(SOURCE).toContain('{state.detail ? <p');
    expect(SOURCE).toContain('{countsLine ? <p');
    expect(SOURCE).toMatch(/\{countsLine \? <p[^>]*>\{countsLine\}<\/p> : null\}/);
    expect(SOURCE).toContain('<FullScanPanelView');
    expect(SOURCE.indexOf('{state.detail ? <p')).toBeLessThan(SOURCE.indexOf('{countsLine ? <p'));
    expect(SOURCE.indexOf('{countsLine ? <p')).toBeLessThan(SOURCE.indexOf('<FullScanPanelView'));
  });

  it('两个排队按钮默认收起，说明在按钮前，暂停留在折叠区外', () => {
    const details = SOURCE.slice(SOURCE.indexOf('<details className="min-w-0 flex-1">'), SOURCE.indexOf('</details>') + '</details>'.length);
    expect(details).toContain('<summary');
    expect(details).not.toMatch(/<details[^>]*\bopen\b/);
    expect(SOURCE).toContain('只想慢慢整理（可选）');
    expect(SLOW_ORGANIZE_COPY.title).toBe('只想慢慢整理（可选）');
    expect(details).toContain('{SLOW_ORGANIZE_COPY.title}');
    expect(details).toContain("enqueueCards('missing')");
    expect(details).toContain("enqueueCards('outdated')");
    expect(details).toContain('{SLOW_ORGANIZE_COPY.missingLabel}');
    expect(details).toContain('{SLOW_ORGANIZE_COPY.outdatedLabel}');
    expect(details.indexOf('{SLOW_ORGANIZE_COPY.description}')).toBeLessThan(details.indexOf('<Button'));
    expect(details).not.toContain('pauseAutomation');
    expect(SOURCE).toContain('onClick={pauseAutomation}>暂停</Button>');
    expect(SOURCE).toContain('setEnqueueNote(enqueueCardsNote(count, unlimited))');
    expect(SOURCE).toContain("invoke<number>('enqueue_cards_now', { scope })");
  });

  it('README 行仍在现有收藏面板之后，每天整理之前；旧文案不再出现', () => {
    expect(SOURCE.indexOf('<FullScanPanelView')).toBeLessThan(SOURCE.indexOf('{readmeLine ?'));
    expect(SOURCE.indexOf('{readmeLine ?')).toBeLessThan(SOURCE.indexOf('{DAILY_ORGANIZE_TITLE}'));
    expect(SOURCE).not.toContain('集中补齐');
    expect(SOURCE).not.toContain('明天会');
    expect(SOURCE).not.toContain('一次性全部整理</');
  });
});
