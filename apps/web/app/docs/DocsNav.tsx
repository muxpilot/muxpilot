"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { DOCS } from "./nav";

// Sidebar for the docs shell. Client component only so it can mark the active
// route with aria-current from the pathname.
export function DocsNav() {
  const pathname = usePathname();
  return (
    <nav className="docs-nav" aria-label="Docs">
      <p className="group">Guide</p>
      <ul>
        {DOCS.map((doc) => (
          <li key={doc.slug}>
            <Link
              href={doc.href}
              aria-current={pathname === doc.href ? "page" : undefined}
            >
              {doc.title}
            </Link>
          </li>
        ))}
      </ul>
    </nav>
  );
}
