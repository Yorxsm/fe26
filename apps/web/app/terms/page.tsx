import type { Metadata } from "next";
import { MarkdownDoc } from "@/components/MarkdownDoc";
import terms from "../../../../TERMS.md";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Terms of Use",
  "Terms for the fe2VI arena, agent wallets, lobby, and War Room.",
  "/terms",
);

export default function TermsPage() {
  return (
    <main className="board terms-page">
      <MarkdownDoc source={terms} />
    </main>
  );
}
