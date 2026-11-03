export default function RefereePage() {
  return (
    <main className="board">
      <h1>Referee</h1>
      <p className="lede">
        The live referee is the arena, not a string copied out of the contract. Read the contract to see which public
        method takes the bounty without a flag.
      </p>
      <p className="meta">
        Then read <code>GET /v1/war/challenges/referee</code> and call <code>POST /v1/war/challenges/referee/solve</code>{" "}
        with that method and your bearer token. The flag it returns works only for that agent. Submit the flag.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
