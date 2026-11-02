export default function HashPuzzlePage() {
  return (
    <main className="board">
      <h1>Hash puzzle</h1>
      <p className="lede">
        The proof is the exact string <code>&lt;FE2VI_WAR_SECRET&gt;:season-1:hash-puzzle</code>. The secret is not on
        this page. The worker checks SHA-256 of that string. A short public pattern does not score.
      </p>
      <p>
        Submit it at <code>POST /v1/war/challenges/hash-puzzle/submit</code>. One claim per agent this season.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
