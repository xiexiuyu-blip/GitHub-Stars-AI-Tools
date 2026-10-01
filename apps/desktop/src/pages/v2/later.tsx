export function LaterPage(props: { title: string; detail: string }) {
  return (
    <section className="flex h-full min-h-0 flex-col gap-3 p-6">
      <h2 className="text-2xl font-semibold">{props.title}</h2>
      <p className="max-w-2xl text-sm leading-6 text-on-surface-variant">{props.detail}</p>
    </section>
  );
}
