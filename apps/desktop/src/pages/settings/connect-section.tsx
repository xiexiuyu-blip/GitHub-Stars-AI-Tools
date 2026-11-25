import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button, useToast } from '@/components/kit';
import { Icon } from '@/components/ui/icon';
import { errorText, Section, TokenRow } from './shared';

type ApiStatus = { enabled: boolean; host: string; port: number; listening: boolean; hasToken: boolean };

export function ConnectSection() {
  const toast = useToast();
  const [api, setApi] = useState<ApiStatus | null>(null);

  function say(title: string) {
    toast.show({ title });
  }

  function loadStatus() {
    invoke<ApiStatus>('get_local_api_status').then(setApi).catch((reason: unknown) => say(errorText(reason)));
  }

  useEffect(loadStatus, []);

  return (
    <div className="space-y-6">
      <Section
        icon="hub"
        hue={268}
        title="本地接口与外部工具接入"
        hint="默认关闭，需要让本机其他程序（例如微信工作台、外部脚本或后续 AI 助手）连进来时再开启。"
        actions={
          <Button
            onClick={() => void invoke<ApiStatus>('set_local_api_enabled', { enabled: !api?.enabled }).then((next) => setApi(next)).catch((reason: unknown) => say(errorText(reason)))}
          >
            {api?.enabled ? '关闭' : '开启'}
          </Button>
        }
      >
        <p className="m-0 text-sm text-on-surface-variant">
          现在：{api?.enabled ? '已开启' : '已关闭'}。开启后，只在本机地址（127.0.0.1:39217）接收请求，局域网或外网访问不到。
        </p>
        {api?.enabled ? (
          <div className="mt-3 flex flex-col gap-2.5">
            <TokenRow
              title="收集专用口令（用于微信工作台等）"
              hint="复制后粘到外部工具的设置里。它只能往「收集」里放链接，看不到你的资料库。"
              onCopy={() => void invoke<string>('get_local_api_capture_token').then((token) => navigator.clipboard.writeText(token)).then(() => say('收集专用口令已复制')).catch((reason: unknown) => say(errorText(reason)))}
              onReset={() => void invoke<string>('rotate_local_api_capture_token').then(() => say('收集专用口令已重置，旧的立刻失效，请把新的粘回对应工具')).catch((reason: unknown) => say(errorText(reason)))}
            />
            <details className="group">
              <summary className="flex cursor-pointer list-none items-center gap-1 text-xs text-on-surface-variant [&::-webkit-details-marker]:hidden">
                <Icon name="chevron_right" size={16} className="transition-transform group-open:rotate-90" />
                高级：能读你资料库的完整令牌
              </summary>
              <div className="mt-2">
                <TokenRow
                  title="完整令牌"
                  hint="这串口令能读你的资料库。只填进你信得过的本机程序，不要发给别人，也不要贴到网页或聊天里。程序怎么填，按它自己的说明来。"
                  onCopy={() => void invoke<string>('get_local_api_token').then((token) => navigator.clipboard.writeText(token)).then(() => say('完整令牌已复制')).catch((reason: unknown) => say(errorText(reason)))}
                  onReset={() => void invoke<string>('rotate_local_api_token').then(() => { say('完整令牌已重置，旧的立即失效'); loadStatus(); }).catch((reason: unknown) => say(errorText(reason)))}
                />
              </div>
            </details>
          </div>
        ) : null}
      </Section>

      <Section
        icon="extension"
        hue={210}
        title="AI 助手接入（MCP，规划预留）"
        hint="后续版本将在此直接提供 Claude Code / Codex / Antigravity 的只读连接配置与调用记录，让外部 AI 助手自动检索你的收藏。"
      >
        <p className="m-0 text-xs leading-[1.6] text-on-surface-variant">
          接入时只复用上面的本机接口，不开放给外部网络，也不会替你调用付费 AI。
        </p>
      </Section>
    </div>
  );
}
