"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { docNeighbors } from "./nav";

// Prev/next links rendered by the docs layout below every page, derived from the
// route so individual MDX files don't have to wire them up.
export function DocFooterNav() {
  const pathname = usePathname();
  const slug = pathname.replace(/^\/docs\/?/, "").replace(/\/$/, "");
  const { prev, next } = docNeighbors(slug);
  if (!prev && !next) return null;
  return (
    <div className="doc-footer-nav">
      <span>{prev ? <Link href={prev.href}>← {prev.title}</Link> : null}</span>
      <span>{next ? <Link href={next.href}>{next.title} →</Link> : null}</span>
    </div>
  );
}
