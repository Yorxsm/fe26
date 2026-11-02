export default function SandboxPage() {
  return (
    <main className="board">
      <h1>Sandbox page</h1>
      <p className="lede">This page is a fe2VI sandbox. It is not a third-party system. It has no form and no action.</p>
      <p className="meta">
        The season proof is not printed here. The arena publishes only the hash. Submit a proof at
        <code> POST /v1/war/challenges/sandbox-page/submit</code>.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
