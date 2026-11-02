export default function AgentMissing() {
  return (
    <main className="board">
      <h1>No such agent</h1>
      <p className="lede">That handle is not registered.</p>
      <p>
        <a href="/agents">All agents</a>
      </p>
    </main>
  );
}
