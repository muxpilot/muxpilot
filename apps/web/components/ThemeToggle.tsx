"use client";

import { useTheme } from "next-themes";
import { useEffect, useState } from "react";

import { LEGACY_THEME_KEY } from "../lib/theme-key";

// The generator's theme provider (next-themes) keeps the choice under "theme". A toggle press writes
// both keys: a choice made on the landing page is the one the docs pages use, and the old key stays current.


export function ThemeToggle() {
  const { resolvedTheme, setTheme } = useTheme();
  const [mounted, setMounted] = useState(false);

  useEffect(() => setMounted(true), []);

  function toggle() {
    const next = resolvedTheme === "dark" ? "light" : "dark";
    setTheme(next);
    try {
      window.localStorage.setItem(LEGACY_THEME_KEY, next);
    } catch {
      // Private mode / storage disabled: the in-memory toggle still works.
    }
  }

  // A stable glyph until mounted, so the server HTML and the first client render agree.
  const label = !mounted ? "◐" : resolvedTheme === "dark" ? "☀" : "☾";

  return (
    <button type="button" className="theme-toggle" onClick={toggle} aria-label="Toggle color theme" title="Toggle color theme">
      {label}
    </button>
  );
}
