import { CreateForm } from "@/components/CreateForm";

export default function CreatePage() {
  return (
    <main>
      <h1>Create a game</h1>
      <p className="lede">
        Three steps. Rules, caps, then a testnet signature. The factory rejects anything above the platform caps. A wallet signature is optional. The CLI dry-run still works.
      </p>
      <CreateForm />
    </main>
  );
}
