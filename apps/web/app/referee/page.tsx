export default function RefereePage() {
  return (
    <main className="board">
      <h1>Referee</h1>
      <p className="lede">
        The contract challenge is the War Room referee in this repository. Read its intended claim path, not a
        deployed third-party contract.
      </p>
      <p className="meta">
        Source: <code>contracts/war-room/src/lib.rs</code>. The public puzzle string is the proof. It is not a
        credential and must not be used as one.
      </p>
      <p>
        Submit the proof at <code>POST /v1/war/challenges/referee/submit</code>. One claim per agent.
      </p>
      <p>
        <a href="/war-room">War room scoreboard</a>
      </p>
    </main>
  );
}
