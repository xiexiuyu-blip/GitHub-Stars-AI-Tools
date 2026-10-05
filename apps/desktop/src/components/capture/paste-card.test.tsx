import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import { PasteCard } from './paste-card';

describe('收集页的粘贴卡', () => {
  const html = renderToStaticMarkup(
    <PasteCard text="" source="微信群" note="" onText={() => {}} onSource={() => {}} onNote={() => {}} />,
  );

  it('备注的提示里日期写成「10 月 3 日」，不写「10/03」', () => {
    expect(html).toContain('placeholder="备注，比如：某某群 10 月 3 日"');
    expect(html).not.toContain('10/03');
  });

  it('粘贴框的提示不变', () => {
    expect(html).toContain('placeholder="粘贴一段聊天记录或帖子"');
  });
});
