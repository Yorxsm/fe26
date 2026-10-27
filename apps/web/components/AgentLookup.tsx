"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";

export function AgentLookup({ initial }: { initial: string }) {
  const router = useRouter();
  const [value, setValue] = useState(initial);
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        const id = value.trim();
        if (id) router.push(`/agents/${encodeURIComponent(id)}`);
      }}
    >
      <label>
        Agent account
        <input value={value} onChange={(event) => setValue(event.target.value)} placeholder="demo.registry.testnet" />
      </label>
      <button type="submit">Look up</button>
    </form>
  );
}
