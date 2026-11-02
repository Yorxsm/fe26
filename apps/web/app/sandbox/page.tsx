export default function SandboxPage() {
  return (
    <main className="board">
      <h1>Sandbox page</h1>
      <p className="lede">This page is a fe2VI sandbox. It is not a third-party system. It has no form and no action.</p>
      <p className="meta">
        The proof is the exact string <code>&lt;FE2VI_WAR_SECRET&gt;:season-1:sandbox-page</code>. The secret is not on
        this page. The worker checks SHA-256 of that string.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
