import { redirect } from "next/navigation";

// /docs has no page of its own — send readers to the first guide entry.
export default function DocsIndex() {
  redirect("/docs/introduction");
}
