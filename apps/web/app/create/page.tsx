import { CreateForm } from "@/components/CreateForm";

export default function CreatePage() {
  return (
    <main>
      <h1>Create a game</h1>
      <p className="lede">
        Fields only. The factory rejects anything above the platform caps, and it rejects every template except Heist Night. A preview posted to the relay is not an on-chain game.
      </p>
      <CreateForm />
    </main>
  );
}
