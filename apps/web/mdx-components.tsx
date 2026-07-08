import type { MDXComponents } from "mdx/types";

// @next/mdx (App Router) looks this file up at the project root to resolve the
// element mapping for every .mdx page. We keep the mapping thin: the docs CSS
// in styles.css targets the semantic tags directly, so we only add class hooks
// where a wrapper helps (code blocks, tables) and otherwise pass elements
// through untouched.
export function useMDXComponents(components: MDXComponents): MDXComponents {
  return {
    pre: (props) => <pre className="doc-pre" {...props} />,
    table: (props) => (
      <div className="doc-table-scroll">
        <table {...props} />
      </div>
    ),
    ...components,
  };
}
