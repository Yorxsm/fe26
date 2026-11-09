import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export async function generateMetadata({
  params,
}: {
  params: Promise<{ tokenId: string }>;
}): Promise<Metadata> {
  const { tokenId } = await params;
  return pageMetadata(
    tokenId,
    "Public passport record. Strategy version count and last update are shown. The playbook is not.",
    `/passports/${encodeURIComponent(tokenId)}`,
  );
}

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
