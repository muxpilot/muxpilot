"use client";

import { useEffect, useState } from "react";

type Theme = "light" | "dark";

// Reads whatever the pre-paint inline script already resolved (see layout.tsx),
// then lets the user flip it. We persist to localStorage and stamp data-theme on
// <html> so the CSS token overrides win over the OS preference.
function currentTheme(): Theme {
  if (typeof document === "undefined") return "light";
  const attr = document.documentElement.getAttribute("data-theme");
  if (attr === "light" || attr === "dark") return attr;
  return window.matchMedia("(prefers-color-scheme: dark)").matches
    ? "dark"
    : "light";
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>("light");
  const [mounted, setMounted] = useState(false);

  useEffect(() => {
    setTheme(currentTheme());
    setMounted(true);
  }, []);

  function toggle() {
    const next: Theme = theme === "dark" ? "light" : "dark";
    setTheme(next);
    document.documentElement.setAttribute("data-theme", next);
    try {
      window.localStorage.setItem("muxpilot-theme", next);
    } catch {
      // Private mode / storage disabled — the in-memory toggle still works.
    }
  }

  // Render a stable placeholder until mounted so SSR and first client paint
  // agree (avoids a hydration mismatch on the icon).
  const label = !mounted ? "◐" : theme === "dark" ? "☀" : "☾";

  return (
    <button
      type="button"
      className="theme-toggle"
      onClick={toggle}
      aria-label="Toggle color theme"
      title="Toggle color theme"
    >
      {label}
    </button>
  );
}
