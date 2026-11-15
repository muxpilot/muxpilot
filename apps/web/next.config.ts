import type { NextConfig } from "next";
import createMDX from "@next/mdx";

const nextConfig: NextConfig = {
  // Let App Router treat .md / .mdx files as routable pages alongside .tsx.
  pageExtensions: ["ts", "tsx", "js", "jsx", "md", "mdx"],
  // The site is a static export (apps/web/out), served by Cloudflare Workers static assets.
  output: "export",
  images: { unoptimized: true },
};

const withMDX = createMDX({
  // Keep the pipeline dependency-light and deterministic — no remote plugins.
  options: {
    remarkPlugins: [],
    rehypePlugins: [],
  },
});

export default withMDX(nextConfig);
