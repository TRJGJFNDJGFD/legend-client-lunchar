import { useEffect } from "react";
import { useAppStore } from "../store/useAppStore";

export function Settings() {
  const { javaInstalls, javaChecked, refreshJava } = useAppStore();

  useEffect(() => {
    refreshJava();
  }, [refreshJava]);

  return (
    <div className="p-8 max-w-2xl">
      <h1 className="text-2xl font-bold mb-6">Settings</h1>

      <section className="mb-8">
        <h2 className="text-sm font-semibold text-[var(--color-text-dim)] uppercase mb-3">
          Java installations detected
        </h2>
        {!javaChecked && <p className="text-sm text-[var(--color-text-dim)]">Scanning…</p>}
        {javaChecked && javaInstalls.length === 0 && (
          <p className="text-sm text-[var(--color-danger)]">No Java installation found.</p>
        )}
        <div className="flex flex-col gap-2">
          {javaInstalls.map((j) => (
            <div key={j.path} className="glass rounded-lg px-4 py-3 flex justify-between text-sm">
              <span className="font-mono">{j.path}</span>
              <span className="text-[var(--color-text-dim)]">
                v{j.version} (Java {j.major}, {j.is_64bit ? "64-bit" : "32-bit"})
              </span>
            </div>
          ))}
        </div>
      </section>

      <section>
        <h2 className="text-sm font-semibold text-[var(--color-text-dim)] uppercase mb-3">
          Other settings categories
        </h2>
        <p className="text-sm text-[var(--color-text-dim)]">
          General / Minecraft / Performance / Graphics / Audio / HUD / Controls / Privacy / Advanced
          — planned, see docs/ROADMAP.md.
        </p>
      </section>
    </div>
  );
}
