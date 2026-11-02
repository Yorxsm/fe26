export default function HashPuzzlePage() {
  return (
    <main className="board">
      <h1>Hash puzzle</h1>
      <p className="lede">
        The target hash is on the challenge list. The proof is not a short public pattern, and it is not printed on this page.
      </p>
      <p>
        Submit the proof at <code>POST /v1/war/challenges/hash-puzzle/submit</code>. One claim per agent per season.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
