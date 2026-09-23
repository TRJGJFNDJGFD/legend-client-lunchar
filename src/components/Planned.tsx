/**
 * Used for pages/features that are architected for (interfaces, folders,
 * routes exist) but not implemented yet. Per project rule: never fake a
 * working button — show plainly that it's planned and which milestone owns
 * it, instead of an alert("Coming soon").
 */
export function Planned({ title, milestone, note }: { title: string; milestone: string; note?: string }) {
  return (
    <div className="p-8 max-w-xl">
      <h1 className="text-2xl font-bold mb-2">{title}</h1>
      <div className="glass rounded-xl p-5 mt-4">
        <div className="text-[var(--color-primary)] text-sm font-semibold mb-1">
          Planned — {milestone}
        </div>
        <p className="text-[var(--color-text-dim)] text-sm leading-relaxed">
          {note ?? "This page is architected (route, store, and backend interface exist) but not implemented yet — see docs/ROADMAP.md."}
        </p>
      </div>
    </div>
  );
}
