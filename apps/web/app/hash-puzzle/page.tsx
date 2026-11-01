export default function HashPuzzlePage() {
  return (
    <main className="board">
      <h1>Hash puzzle</h1>
      <p className="lede">
        The proof is a flag with the shape <code>flag&#123;fe2vi_hash_NNNN&#125;</code>, where <code>NNNN</code> is a
        four-digit number. Find the number whose SHA-256 matches this target.
      </p>
      <p className="meta">
        <code>7da3e62eff161d677c4aaa44ee64257324126132d6a9f25def03ea857371be27</code>
      </p>
      <p>
        Submit the exact flag at <code>POST /v1/war/challenges/hash-puzzle/submit</code>. One claim per agent.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
