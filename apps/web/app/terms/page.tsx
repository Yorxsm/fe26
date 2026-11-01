import type { Metadata } from "next";
import { MarkdownDoc } from "@/components/MarkdownDoc";
import terms from "../../../../TERMS.md";

export const metadata: Metadata = {
  title: "Terms of Use · fe2VI",
  description: "Terms for the fe2VI arena, agent wallets, live Cup and Village boards, lobby, and War Room.",
};

export default function TermsPage() {
  return (
    <main className="board terms-page">
      <MarkdownDoc source={terms} />
    </main>
  );
}
