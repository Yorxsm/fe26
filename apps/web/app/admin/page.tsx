"use client";

import { useEffect, useState } from "react";

type Status = {
  api: { ok: boolean; status: number; latency_ms: number };
  cron_last: number | null;
  errors: number | null;
  requests: number | null;
  durable_object: { games: number; agents: number; events: number; indexed_height: number } | null;
};

export default function AdminPage() {
  const [status, setStatus] = useState<Status | null>(null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    fetch("/api/admin/status", { cache: "no-store" })
      .then((response) => (response.ok ? response.json() : Promise.reject(new Error("missing"))))
      .then((body) => setStatus(body as Status))
      .catch(() => setFailed(true));
  }, []);
  return (
    <main className="board">
      <h1>Admin</h1>
      <p>
        <a href="/admin/styles">Styles</a>
      </p>
      {failed ? <p className="meta">Live data unavailable</p> : null}
      <section className="card">
        <h2>API health</h2>
        <p className="num">
          {status ? `${status.api.ok ? "ok" : "down"} · ${status.api.status} · ${status.api.latency_ms} ms` : "…"}
        </p>
      </section>
      <section className="card">
        <h2>Cron last run</h2>
        <p className="num">{status?.cron_last ? new Date(status.cron_last * 1000).toISOString() : "…"}</p>
      </section>
      <section className="card">
        <h2>Durable Object</h2>
        <p className="num">
          {status?.durable_object
            ? `${status.durable_object.games} games · ${status.durable_object.agents} agents · ${status.durable_object.events} events · height ${status.durable_object.indexed_height}`
            : "…"}
        </p>
      </section>
      <section className="card">
        <h2>Error count</h2>
        <p className="num">{status ? `${status.errors ?? 0} errors · ${status.requests ?? 0} requests` : "…"}</p>
      </section>
    </main>
  );
}
