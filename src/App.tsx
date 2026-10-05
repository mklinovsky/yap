import { useState, type ReactNode } from "react";
import { History } from "./History";
import { Settings } from "./Settings";
import { TryIt } from "./TryIt";

export type Section = "history" | "try" | "settings";

const sections: { id: Section; title: string; icon: ReactNode }[] = [
  {
    id: "settings",
    title: "Settings",
    icon: (
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="2.5" />
        <path d="M10 2.75v2M10 15.25v2M2.75 10h2M15.25 10h2M4.87 4.87l1.42 1.42M13.71 13.71l1.42 1.42M4.87 15.13l1.42-1.42M13.71 6.29l1.42-1.42" />
      </svg>
    ),
  },
  {
    id: "history",
    title: "History",
    icon: (
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="7.25" />
        <path d="M10 5.5V10l3 2" />
      </svg>
    ),
  },
  {
    id: "try",
    title: "Try it",
    icon: (
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <rect x="7.25" y="2.75" width="5.5" height="9.5" rx="2.75" />
        <path d="M4.75 9.5a5.25 5.25 0 0 0 10.5 0M10 14.75v2.5" />
      </svg>
    ),
  },
];

export function App({ initialSection }: { initialSection: Section }) {
  const [section, setSection] = useState<Section>(initialSection);
  const current = sections.find((s) => s.id === section) ?? sections[0];

  return (
    <div className="app">
      <nav className="sidebar" data-tauri-drag-region>
        <div className="sidebar-title" data-tauri-drag-region>
          yap
        </div>
        {sections.map((s) => (
          <button
            key={s.id}
            type="button"
            className="nav-item"
            aria-current={s.id === section ? "page" : undefined}
            onClick={() => setSection(s.id)}
          >
            {s.icon}
            {s.title}
          </button>
        ))}
      </nav>
      <main className="content">
        <header className="content-header" data-tauri-drag-region>
          <h1>{current.title}</h1>
        </header>
        <div className="content-body" data-section={section}>
          {section === "history" && <History />}
          {section === "try" && <TryIt />}
          {section === "settings" && <Settings />}
        </div>
      </main>
    </div>
  );
}
