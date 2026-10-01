import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

type Hit = { id: string; fullName: string; why: string };
type Answer = { owned: Hit[]; gaps: string[]; external: Hit[] };

const EXAMPLES = ['我想整理群聊里的工具链接', '本地知识库和向量检索', '做 PPT 和文档'];

export function SolvePage(props: { initialQuestion?: string; onCreatePack: (question: string) => void }) {
  const [question, setQuestion] = useState(props.initialQuestion || EXAMPLES[0]);
  const [answer, setAnswer] = useState<Answer | null>(null);
  useEffect(() => {
    if (props.initialQuestion) setQuestion(props.initialQuestion);
  }, [props.initialQuestion]);
  return (
    <section className="flex h-full flex-col gap-4 overflow-auto p-6">
      <h2 className="text-2xl font-semibold">找方案</h2>
      <div className="flex flex-wrap gap-2">
        {EXAMPLES.map((example) => <button key={example} className="rounded-full border px-3 py-1 text-xs" type="button" onClick={() => setQuestion(example)}>{example}</button>)}
      </div>
      <textarea className="min-h-24 rounded-2xl border p-4 text-sm" value={question} onChange={(event) => setQuestion(event.target.value)} />
      <button className="w-fit rounded-full bg-primary px-4 py-2 text-sm text-on-primary" type="button" onClick={() => void invoke<Answer>('solve_problem', { question }).then(setAnswer)}>查找</button>
      {answer ? (
        <div className="grid gap-4 lg:grid-cols-3">
          <Column title="你已经有的" items={answer.owned.map((item) => `${item.fullName}：${item.why}`)} />
          <Column title="还缺的" items={answer.gaps} />
          <Column title="外面更好的" items={answer.external.length ? answer.external.map((item) => item.fullName) : ['榜单快照还是空的，先去发现页看默认来源。']} />
        </div>
      ) : null}
      <button className="w-fit rounded-full border px-4 py-2 text-sm" type="button" onClick={() => props.onCreatePack(question)}>用这个问题生成参考包</button>
    </section>
  );
}

function Column(props: { title: string; items: string[] }) {
  return (
    <div className="rounded-2xl border p-4">
      <h3 className="font-medium">{props.title}</h3>
      <ul className="mt-2 space-y-2 text-sm text-on-surface-variant">{props.items.map((item) => <li key={item}>{item}</li>)}</ul>
    </div>
  );
}
