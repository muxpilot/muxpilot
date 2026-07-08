import type { Metadata } from "next";
import { SiteHeader } from "../components/SiteHeader";
import { SiteFooter } from "../components/SiteFooter";
import { DocsNav } from "./DocsNav";
import { DocFooterNav } from "./DocFooterNav";

export const metadata: Metadata = {
  title: {
    template: "%s · MuxPilot docs",
    default: "MuxPilot docs",
  },
};

export default function DocsLayout({
  children,
}: Readonly<{ children: React.ReactNode }>) {
  return (
    <>
      <SiteHeader />
      <div className="docs">
        <DocsNav />
        <article className="doc">
          {children}
          <DocFooterNav />
        </article>
      </div>
      <SiteFooter />
    </>
  );
}
