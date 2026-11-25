import { describe, expect, it } from 'vitest';
import SOURCE from '@/pages/settings/organize-section.tsx?raw';

describe('自动整理页：确认入口和纯函数接线', () => {
  const panelSource = SOURCE.slice(SOURCE.indexOf('export function FullScanPanelView('), SOURCE.indexOf('export function OrganizeSection('));

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

  it('收口后移除容易混淆的手动排队折叠区，只保留「现在整理」与「每天自动整理」', () => {
    expect(SOURCE).not.toContain('enqueue_cards_now');
    expect(SOURCE).not.toContain('只想慢慢整理');
    expect(SOURCE).toContain('onClick={pauseAutomation}>暂停</Button>');
  });

  it('指标卡正文不再塞模型名，旧文案不再出现', () => {
    expect(SOURCE).toContain('sub={`进行中 ${automation.running} · 失败 ${automation.failed}`}');
    expect(SOURCE).not.toContain('集中补齐');
    expect(SOURCE).not.toContain('明天会');
    expect(SOURCE).not.toContain('一次性全部整理</');
  });
});
