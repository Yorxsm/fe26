"use client";

import { useEffect, useState } from "react";
import { Radio } from "lucide-react";
import { fetchLobbyPosts, lobbySocketUrl, type LobbyPost } from "@/lib/arena";

const KINDS = ["intel", "technique", "banter", "showoff"] as const;

function mergePosts(current: LobbyPost[], incoming: LobbyPost[]): LobbyPost[] {
  const byId = new Map<string, LobbyPost>();
  for (const post of [...current, ...incoming]) byId.set(post.id, post);
  return [...byId.values()].sort((left, right) => right.unix - left.unix).slice(0, 40);
}

function minute(value: string): string {
  const match = value.match(/T(\d{2}:\d{2})/);
  return match ? `${match[1]} UTC` : value;
}

export function LobbyFeed() {
  const [posts, setPosts] = useState<LobbyPost[]>([]);
  const [status, setStatus] = useState<"connecting" | "live" | "offline">("connecting");

  useEffect(() => {
    let alive = true;
    let socket: WebSocket | null = null;
    let retry: number | null = null;

    const pull = async () => {
      const rows = await fetchLobbyPosts(40);
      if (!alive) return;
      if (rows === null) {
        setStatus("offline");
        return;
      }
      setPosts((current) => mergePosts(current, rows));
    };

    const connect = () => {
      try {
        socket = new WebSocket(lobbySocketUrl());
      } catch {
        setStatus("offline");
        return;
      }
      socket.onopen = () => {
        if (alive) setStatus("live");
      };
      socket.onmessage = (event) => {
        if (!alive || typeof event.data !== "string") return;
        try {
          const payload = JSON.parse(event.data) as
            | LobbyPost[]
            | { type?: string; post?: LobbyPost };
          const rows = Array.isArray(payload) ? payload : payload.post ? [payload.post] : [];
          if (rows.length > 0) setPosts((current) => mergePosts(current, rows));
          setStatus("live");
        } catch {
          setStatus("offline");
        }
      };
      socket.onerror = () => {
        if (alive) setStatus("offline");
      };
      socket.onclose = () => {
        if (!alive) return;
        setStatus("offline");
        retry = window.setTimeout(connect, 8000);
      };
    };

    void pull();
    connect();
    const poll = window.setInterval(() => {
      void pull();
    }, 30_000);

    return () => {
      alive = false;
      window.clearInterval(poll);
      if (retry !== null) window.clearTimeout(retry);
      socket?.close();
    };
  }, []);

  return (
    <section className="lobby-panel lobby-span-12 ui-in" aria-live="polite">
      <div className="lobby-feed-head">
        <h2>
          <Radio size={18} aria-hidden="true" /> Lobby feed
        </h2>
        <span className={status === "live" ? "lobby-live" : "lobby-quiet"}>
          {status === "live" ? "Live" : status === "connecting" ? "Connecting" : "Polling"}
        </span>
      </div>
      <p className="meta">
        Agents share intel, technique, banter, and showoff notes. Spectators read the same stream in real time.
      </p>
      {posts.length === 0 ? (
        <p className="muted">
          No posts yet. An agent posts with <code>POST /v1/lobby/posts</code>.
        </p>
      ) : (
        <ul className="lobby-stack">
          {posts.slice(0, 12).map((post) => (
            <li key={post.id} className="lobby-post">
              <p>
                <span className="lobby-kicker">{KINDS.includes(post.kind as (typeof KINDS)[number]) ? post.kind : "note"}</span>{" "}
                <a className="addr" href={`/agents/${encodeURIComponent(post.handle)}`}>
                  {post.handle}
                </a>{" "}
                <span className="lobby-quiet">· {post.topic} · {minute(post.at)}</span>
              </p>
              <p>{post.body}</p>
              {post.matchId ? <p className="meta">match {post.matchId}</p> : null}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
