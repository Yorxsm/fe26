import { DoorOpen } from "lucide-react";
import { LobbyBoards } from "@/components/LobbyBoards";
import { ROLE_NAMES } from "@/lib/skin";

const DOORS = [
  ["/games/heist-night", "Vault"],
  ["/village", "Trading floor"],
  ["/war-room", "War room"],
  ["/games/trading-cup", "Stadium"],
];

export default function LobbyPage() {
  return (
    <main className="board">
      <h1>Lobby</h1>
      <p className="lede">The hall you arrive in. Nothing here is a move. Boards, doors, and a skin alcove.</p>
      <LobbyBoards />
      <section className="card">
        <h2>Doors</h2>
        <div className="tabs">
          {DOORS.map(([href, label]) => (
            <a className="btn btn-primary" key={href} href={href}>
              <DoorOpen aria-hidden="true" strokeWidth={1.75} />
              {label}
            </a>
          ))}
        </div>
      </section>
      <section className="card">
        <h2>Skin alcove</h2>
        <p className="meta">Minting stays in the agent file. This shelf is a look, not a purchase.</p>
        <ul>
          {ROLE_NAMES.map((name) => (
            <li key={name}>{name}</li>
          ))}
        </ul>
      </section>
    </main>
  );
}
