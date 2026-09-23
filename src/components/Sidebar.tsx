import { NavLink } from "react-router-dom";

const NAV_ITEMS: { to: string; label: string; icon: string }[] = [
  { to: "/", label: "Home", icon: "🏠" },
  { to: "/profiles", label: "Profiles", icon: "📁" },
  { to: "/mods", label: "Mods", icon: "🧩" },
  { to: "/modpacks", label: "Modpacks", icon: "📦" },
  { to: "/resource-packs", label: "Resource Packs", icon: "🎨" },
  { to: "/shaders", label: "Shaders", icon: "✨" },
  { to: "/cosmetics", label: "Cosmetics", icon: "🎭" },
  { to: "/servers", label: "Servers", icon: "🌐" },
  { to: "/screenshots", label: "Screenshots", icon: "📷" },
  { to: "/logs", label: "Logs", icon: "📜" },
];

export function Sidebar() {
  return (
    <aside className="w-56 shrink-0 h-full flex flex-col justify-between bg-[var(--color-panel)] border-r border-[var(--color-border)]">
      <div>
        <div className="px-5 py-5 text-lg font-bold tracking-wide">
          LEGEND<span className="text-[var(--color-primary)]">CLIENT</span>
        </div>
        <nav className="flex flex-col gap-0.5 px-2">
          {NAV_ITEMS.map((item) => (
            <NavLink
              key={item.to}
              to={item.to}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                  isActive
                    ? "bg-[var(--color-panel-2)] text-[var(--color-text)]"
                    : "text-[var(--color-text-dim)] hover:bg-[var(--color-panel-2)] hover:text-[var(--color-text)]"
                }`
              }
            >
              <span>{item.icon}</span>
              <span>{item.label}</span>
            </NavLink>
          ))}
        </nav>
      </div>

      <div className="flex flex-col gap-0.5 px-2 pb-4">
        <NavLink
          to="/accounts"
          className={({ isActive }) =>
            `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
              isActive
                ? "bg-[var(--color-panel-2)] text-[var(--color-text)]"
                : "text-[var(--color-text-dim)] hover:bg-[var(--color-panel-2)] hover:text-[var(--color-text)]"
            }`
          }
        >
          <span>👤</span>
          <span>Account</span>
        </NavLink>
        <NavLink
          to="/settings"
          className={({ isActive }) =>
            `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
              isActive
                ? "bg-[var(--color-panel-2)] text-[var(--color-text)]"
                : "text-[var(--color-text-dim)] hover:bg-[var(--color-panel-2)] hover:text-[var(--color-text)]"
            }`
          }
        >
          <span>⚙️</span>
          <span>Settings</span>
        </NavLink>
      </div>
    </aside>
  );
}
