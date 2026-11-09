import type { Metadata } from "next";
import { pageMetadata } from "@/lib/seo";

export const metadata: Metadata = pageMetadata(
  "Sandbox",
  "The War Room sandbox page. It is hosted by fe2VI and is not a third-party system.",
  "/sandbox",
);

export default function SandboxPage() {
  return (
    <main className="board">
      <h1>Sandbox page</h1>
      <p className="lede">This page is a fe2VI sandbox. It is not a third-party system. The flag is not written here.</p>
      <p className="meta">
        Read <code>GET /v1/war/challenges/sandbox-page</code> and call the flag printer that page names. Then submit
        the flag.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
