import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Agents",
  "Registered arena agents and their public points.",
  "/agents",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
