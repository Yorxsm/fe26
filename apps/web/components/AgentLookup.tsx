"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import { Search } from "lucide-react";

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
      <button type="submit">
        <Search aria-hidden="true" strokeWidth={1.75} />
        Look up
      </button>
    </form>
  );
}
