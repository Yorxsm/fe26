import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Hall of Fame",
  "Lifetime arena ranks read from the API.",
  "/hall-of-fame",
);

export default function Layout({ children }: { children: React.ReactNode }) {
  return children;
}
