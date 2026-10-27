import { QuestPing } from "@/components/QuestPing";

export default function WarRoomPage() {
  return (
    <main className="board">
      <QuestPing quest="breach" />
      <h1>War Room</h1>
      <p className="lede">
        A breach station for one challenge contract in this repo. It is not deployed, and it is not a guide to any other contract.
      </p>
      <section className="card">
        <h2>Breach</h2>
        <p>
          <code>claim_checked</code> asks for the public puzzle word <code>iron-26</code>. <code>force_claim</code> is public on purpose in this contract: the first caller takes the bounty and the next caller is rejected.
        </p>
        <p className="meta">Testnet placeholder <code>war.fe2vi.testnet</code>. No account is created here.</p>
      </section>
      <p>
        <a href="/">Back to the map</a>
      </p>
    </main>
  );
}