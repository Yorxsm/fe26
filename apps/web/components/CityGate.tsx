"use client";

import dynamic from "next/dynamic";
import { useEffect, useMemo, useRef, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { AudioHud } from "@/components/AudioHud";
import { Lobby } from "@/components/Lobby";
import { CountdownChip } from "@/components/SpectatorBoards";
import { Minimap, QuestLog, Tick } from "@/components/ui";
import { audio } from "@/lib/audio";
import { HomeChips } from "@/components/HomeChips";
import { Logo } from "@/components/Logo";
import { MarkdownDoc } from "@/components/MarkdownDoc";
import { LobbySignList, RoomHud, type RoomMatch, type WarClaimRow } from "@/components/RoomHud";
import {
  ArrowLeft,
  Compass,
  DoorOpen,
  Eye,
  Gamepad2,
  Home,
  Trophy,
  ListOrdered,
  Pause,
  Play,
  MapPin,
  RotateCw,
  ScrollText,
  Send,
  Stamp,
  Store,
  Users,
  Video,
  Volume2,
  X,
} from "lucide-react";
import { LoadingState } from "@/components/States";
import { DISTRICTS, liveStands, markRoomTransition, type CityAgent, type District } from "@/lib/city";
import { loadShownFloor, pendingLoadSteps, rememberLoadFloor } from "@/lib/loadProgress";
import { fetchAgents } from "@/lib/agents";
import { displayName } from "@/lib/avatar";
import { apiBase } from "@/lib/protocol";
import { defaultSkin } from "@/lib/skin";
import {
  EMPTY_SAVE,
  QUEST_DEFS,
  TUTORIAL,
  loadSpectator,
  saveSpectator,
  type SpectatorSave,
} from "@/lib/onboarding";

const CityMap = dynamic(() => import("@/components/CityMap"), { ssr: false });

type Phase = "boot" | "title" | "dispatch" | "city";

function webglAvailable() {
  try {
    const canvas = document.createElement("canvas");
    const options = { failIfMajorPerformanceCaveat: false, powerPreference: "default" as const };
    const gl = (canvas.getContext("webgl2", options) ||
      canvas.getContext("webgl", options)) as WebGLRenderingContext | null;
    return Boolean(gl);
  } catch {
    return false;
  }
}

const ROOM_DEPTH = "fe2vi.roomDepth";
const LOAD_GIVE_UP_MS = 20_000;

function roomDepth() {
  return Number(sessionStorage.getItem(ROOM_DEPTH) || "0") || 0;
}

function retreatRoom() {
  if (roomDepth() > 0) {
    window.history.back();
    return;
  }
  window.history.replaceState({ room: null, fe2vi: true }, "", "/");
}

const ROOM_SLUG: Record<string, string> = {
  "vault-quarter": "vault",
  "cup-stadium": "cup",
  "trading-village": "village",
  "war-room": "war",
  "skin-forge": "lobby",
  "oracle-plaza": "hall",
};

const HUD_LINKS = [
  ["/village", "Village", Home],
  ["/market", "Market", Store],
  ["/forge", "Forge", DoorOpen],
  ["/lobby", "Lobby", DoorOpen],
  ["/agents", "Agents", Users],
] as const;

export function CityGate({ onboarding }: { onboarding: string }) {
  const router = useRouter();
  const path = usePathname();
  const [phase, setPhase] = useState<Phase>("boot");
  const [visitors, setVisitors] = useState<CityAgent[]>([]);
  const [activityLine, setActivityLine] = useState("Reading the arena.");
  const [matches, setMatches] = useState<RoomMatch[] | null>(null);
  const [queues, setQueues] = useState<Record<string, string[]>>({});
  const [claims, setClaims] = useState<WarClaimRow[]>([]);
  const heard = useRef("");
  const [webgl, setWebgl] = useState(true);
  const [save, setSave] = useState<SpectatorSave>(EMPTY_SAVE);
  const [paused, setPaused] = useState(false);
  const [focusId, setFocusId] = useState<string | null>(null);
  const [shotLock, setShotLock] = useState(false);
  const [hotspot, setHotspot] = useState<string | null>(null);
  const [roomBusy, setRoomBusy] = useState(true);
  const [frameMode, setFrameMode] = useState<"in" | "out">("in");
  const [lineup, setLineup] = useState(false);
  const [street, setStreet] = useState(false);
  const [poster, setPoster] = useState<string | null>(null);
  const [followIndex, setFollowIndex] = useState<number | null>(null);
  const [soundLive, setSoundLive] = useState(false);
  const [showGames, setShowGames] = useState(false);
  const [note, setNote] = useState("Click a room or an agent.");
  const [hintsOpen, setHintsOpen] = useState(false);
  const [narrow, setNarrow] = useState(false);
  const [questOpen, setQuestOpen] = useState(false);
  const [shownProgress, setShownProgress] = useState<number | null>(loadShownFloor() || null);
  const [mapReady, setMapReady] = useState(false);
  const [mapHeard, setMapHeard] = useState(false);
  const [mapStuck, setMapStuck] = useState(false);

  useEffect(() => {
    const loaded = loadSpectator();
    setSave(loaded);
    const frame = new URLSearchParams(window.location.search).get("frame");
    if (frame === "street") {
      setStreet(true);
      setPoster("street");
      setPhase("city");
      setNote("The street, after sunset.");
      return;
    }
    if (frame === "lineup" || frame === "face" || frame === "trails" || frame === "skins" || frame === "walk" || frame === "expressions" || frame === "eyes" || frame === "gear" || frame === "front6" || frame === "q34") {
      if (frame === "lineup") setLineup(true);
      setPoster(frame);
      setPaused(true);
      setPhase("city");
      setNote(frame === "lineup" ? "Six roles, on the lawn." : frame);
      return;
    }
    if (frame) {
      const outside = frame.endsWith("-out");
      const id = outside ? frame.slice(0, -4) : frame;
      const framed = DISTRICTS.find((district) => district.id === id);
      if (framed) {
        setShotLock(true);
        setFrameMode(outside ? "out" : "in");
        setFocusId(id);
        setPhase("city");
        setNote(outside ? `${framed.name}, from the lawn.` : `${framed.name}, inside.`);
        return;
      }
    }
    const slug = new URLSearchParams(window.location.search).get("room");
    const fromSlug = slug ? Object.entries(ROOM_SLUG).find(([, value]) => value === slug)?.[0] : null;
    if (fromSlug) {
      selectRoom(fromSlug);
      return;
    }
    if (window.location.hash === "#title") {
      setPhase("title");
      return;
    }
    setPhase(loaded.entered ? "city" : "title");
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) setPaused(true);
    setWebgl(webglAvailable());
    const base = process.env.NEXT_PUBLIC_FE2VI_API_URL?.replace(/\/$/, "");
    if (!base) return;
    fetch(`${base}/v1/schedule`, { cache: "no-store" })
      .then((response) => response.json())
      .then((body) => setRoomBusy(Boolean(body.live)))
      .catch(() => setRoomBusy(false));
  }, []);

  useEffect(() => {
    const showTitle = () => {
      setPhase("title");
      setFocusId(null);
      setFollowIndex(null);
    };
    const onHash = () => {
      if (window.location.hash === "#title") showTitle();
    };
    const onPop = () => {
      const depth = roomDepth();
      if (depth > 0) sessionStorage.setItem(ROOM_DEPTH, String(depth - 1));
      const slug = new URLSearchParams(window.location.search).get("room");
      const id = slug ? Object.entries(ROOM_SLUG).find(([, value]) => value === slug)?.[0] : null;
      if (id) {
        selectRoom(id);
      } else {
        setFrameMode("out");
        window.setTimeout(() => setFocusId(null), 700);
      }
    };
    window.addEventListener("popstate", onPop);
    window.addEventListener("fe2vi-title", showTitle);
    window.addEventListener("hashchange", onHash);
    return () => {
      window.removeEventListener("popstate", onPop);
      window.removeEventListener("fe2vi-title", showTitle);
      window.removeEventListener("hashchange", onHash);
    };
  }, []);

  useEffect(() => {
    const base = apiBase();
    let dead = false;
    let crewKey = "";
    let matchKey = "";
    let lineKey = "";
    const pull = () => {
      const matchesPromise = fetch(`${base}/v1/matches?limit=20`, { cache: "no-store" })
        .then((response) => (response.ok ? response.json() : null))
        .catch(() => null);
      const namesPromise = fetchAgents();
      const listingsPromise = fetch(`${base}/v1/market/listings`, { cache: "no-store" })
        .then((response) => (response.ok ? response.json() : null))
        .catch(() => null);
      const settledPromise = fetch(`${base}/v1/vault/settlements`, { cache: "no-store" })
        .then((response) => (response.ok ? response.json() : null))
        .catch(() => null);
      Promise.all([matchesPromise, namesPromise, listingsPromise, settledPromise]).then(([body, roster, listings, settled]) => {
        if (dead) return;
        const feed = body as { matches?: RoomMatch[]; queues?: Record<string, string[]>; claims?: WarClaimRow[] } | null;
        const next = Array.isArray(feed?.matches) ? feed.matches : [];
        const nextQueues = feed?.queues && typeof feed.queues === "object" ? feed.queues : {};
        const nextClaims = Array.isArray(feed?.claims) ? feed.claims : [];
        if (feed) {
          const live = next.find((match) => match.status === "live" && match.last);
          const token = live?.last ? `${live.matchId}:${live.last.handle}:${live.last.type}:${live.turn}` : "";
          if (token && token !== heard.current) {
            heard.current = token;
            audio.click();
          }
          const nextMatchKey = JSON.stringify([next, nextQueues, nextClaims]);
          if (nextMatchKey !== matchKey) {
            matchKey = nextMatchKey;
            setQueues(nextQueues);
            setClaims(nextClaims);
            setMatches(next);
          }
        } else {
          setMatches((current) => current ?? []);
        }
        const names = new Map<string, string>();
        if (roster.source === "arena") {
          for (const row of roster.agents) {
            if (row.handle) names.set(row.handle, row.name || row.handle);
          }
        }
        const placed: Array<{ handle: string; arenaId: string }> = [];
        for (const match of next) {
          if (match.status !== "live") continue;
          for (const seat of match.seats) placed.push({ handle: seat, arenaId: match.arenaId });
        }
        for (const [arenaId, handles] of Object.entries(nextQueues)) {
          for (const handle of handles) placed.push({ handle, arenaId });
        }
        for (const claim of nextClaims) placed.push({ handle: claim.handle, arenaId: "war" });
        const finished = next.find((match) => match.status !== "live" && match.seats.length > 0);
        if (finished) {
          for (const seat of finished.seats) placed.push({ handle: seat, arenaId: finished.arenaId });
        }
        const stands = liveStands(placed);
        const crew: CityAgent[] = [...stands.entries()].map(([handle, stand]) => ({
          accountId: handle,
          name: names.get(handle) || displayName(handle),
          skin: defaultSkin(handle),
          stand,
        }));
        const key = crew.map((agent) => `${agent.accountId}@${agent.stand?.room}:${agent.stand?.x.toFixed(2)}`).join("|");
        if (key !== crewKey) {
          crewKey = key;
          setVisitors(crew);
        }
        const listed = listings && Array.isArray((listings as { listings?: unknown[] }).listings)
          ? (listings as { listings: unknown[] }).listings.length
          : null;
        const receipts = settled && Array.isArray((settled as { matches?: unknown[] }).matches)
          ? (settled as { matches: unknown[] }).matches.length
          : null;
        const market = listed === null ? "NFT market unread" : listed === 0 ? "NFT market: no listings" : `NFT market: ${listed} listed`;
        const vault = receipts === null ? "Vault receipts unread" : receipts === 0 ? "No settled Vault receipt" : `${receipts} settled Vault receipt${receipts === 1 ? "" : "s"}`;
        const presence = !feed ? "Arena unread" : crew.length === 0 ? "No live activity" : `${crew.length} live`;
        const nextLine = `${presence}. ${market}. ${vault}.`;
        if (nextLine !== lineKey) {
          lineKey = nextLine;
          setActivityLine(nextLine);
        }
      });
    };
    pull();
    const timer = window.setInterval(pull, 8000);
    return () => {
      dead = true;
      window.clearInterval(timer);
    };
  }, []);

  useEffect(() => {
    if (!mapReady) return;
    const familyOf = (variable: string) =>
      getComputedStyle(document.documentElement).getPropertyValue(variable).split(",")[0]?.trim().replace(/^['"]|['"]$/g, "") ?? "";
    const body = familyOf("--font-body");
    const mono = familyOf("--font-mono");
    const faces: Array<[string, string, string]> = [
      [body, "/fonts/Inter-Regular.ttf", "400"],
      [body, "/fonts/Inter-SemiBold.ttf", "600"],
      [body, "/fonts/Inter-Bold.ttf", "700"],
      [mono, "/fonts/JetBrainsMono-Regular.ttf", "400"],
    ];
    for (const [family, url, weight] of faces) {
      if (!family) continue;
      const face = new FontFace(family, `url(${url})`, { weight, style: "normal" });
      face.load().then((loaded) => document.fonts.add(loaded)).catch(() => undefined);
    }
  }, [mapReady]);

  useEffect(() => {
    const queue: number[] = [];
    let finished = false;
    let locked = false;
    let maxSeen = 0;
    let readyTimer = 0;
    const push = (detail: { loaded?: number; total?: number; active?: boolean; progress?: number }) => {
      if (locked) return;
      const total = typeof detail.total === "number" ? detail.total : 0;
      const loaded = typeof detail.loaded === "number" ? detail.loaded : 0;
      let pct = total > 0 ? (loaded / total) * 100 : typeof detail.progress === "number" ? detail.progress : 0;
      pct = Math.max(maxSeen, pct);
      maxSeen = pct;
      queue.push(pct);
      setMapHeard(true);
      setMapStuck(false);
      if (detail.active === false && total > 0 && loaded >= total) {
        locked = true;
        finished = true;
        queue.push(100);
      }
    };
    pendingLoadSteps().forEach(push);
    const onLoad = (event: Event) => {
      const detail = (event as CustomEvent<{ loaded?: number; total?: number; active?: boolean; progress?: number }>).detail;
      if (detail) push(detail);
    };
    window.addEventListener("fe2vi-load", onLoad);
    let frame = 0;
    const drain = () => {
      if (queue.length) {
        const next = queue.shift() ?? 0;
        setShownProgress((current) => {
          const nextValue = Math.max(loadShownFloor(), current ?? 0, Math.round(next));
          if (nextValue > 0) rememberLoadFloor(nextValue);
          return nextValue > 0 ? nextValue : current;
        });
        frame = window.requestAnimationFrame(drain);
        return;
      }
      if (finished) {
        readyTimer = window.setTimeout(() => {
          setMapReady(true);
          window.dispatchEvent(new Event("fe2vi-playable"));
        }, 450);
        return;
      }
      frame = window.requestAnimationFrame(drain);
    };
    frame = window.requestAnimationFrame(drain);
    return () => {
      window.cancelAnimationFrame(frame);
      window.clearTimeout(readyTimer);
      window.removeEventListener("fe2vi-load", onLoad);
    };
  }, []);

  useEffect(() => {
    if (mapReady || !webgl || mapHeard) return;
    const timer = window.setTimeout(() => setMapStuck(true), LOAD_GIVE_UP_MS);
    return () => window.clearTimeout(timer);
  }, [mapReady, webgl, mapHeard]);

  useEffect(() => {
    const query = window.matchMedia("(max-width: 800px)");
    const apply = () => setNarrow(query.matches);
    apply();
    query.addEventListener("change", apply);
    return () => query.removeEventListener("change", apply);
  }, []);

  useEffect(() => {
    document.documentElement.dataset.holdCamera = showGames || hintsOpen || questOpen ? "1" : "";
    return () => {
      delete document.documentElement.dataset.holdCamera;
    };
  }, [showGames, hintsOpen, questOpen]);

  useEffect(() => {
    if (phase === "city") audio.city();
  }, [phase]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setFollowIndex(null);
      setShowGames(false);
      if (new URLSearchParams(window.location.search).get("room")) {
        setFrameMode("out");
        window.setTimeout(() => {
          setFocusId(null);
          setFrameMode("in");
        }, 1200);
        retreatRoom();
        return;
      }
      setFocusId(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  function commit(next: SpectatorSave) {
    setSave(next);
    saveSpectator(next);
  }

  function selectRoom(id: string) {
    const district = DISTRICTS.find((entry) => entry.id === id);
    if (district?.status === "soon") {
      setPhase("city");
      setFocusId(null);
      setNote(`${district.name} is coming soon. Nothing here is live yet.`);
      return;
    }
    setPhase("city");
    setFrameMode("in");
    setFocusId(id);
    setFollowIndex(null);
  }

  function enterCity(nextFocus: string | null) {
    const next = { ...save, entered: true };
    commit(next);
    setFocusId(nextFocus);
    setFollowIndex(null);
    setPhase("city");
  }

  function rememberRoom(id: string | null) {
    const url = new URL(window.location.href);
    if (id && ROOM_SLUG[id]) url.searchParams.set("room", ROOM_SLUG[id]);
    else url.searchParams.delete("room");
    window.history.pushState({ room: id, fe2vi: true }, "", url);
    if (id) sessionStorage.setItem(ROOM_DEPTH, String(roomDepth() + 1));
  }

  function leaveRoom() {
    const flash = document.getElementById("pick-flash");
    if (flash) flash.hidden = true;
    setHotspot(null);
    markRoomTransition();
    audio.crowd(false);
    setFrameMode("out");
    window.setTimeout(() => {
      setFocusId(null);
      setFrameMode("in");
    }, 1200);
    if (new URLSearchParams(window.location.search).get("room")) retreatRoom();
  }

  function pick(district: District) {
    if (district.id === "nft-market") {
      window.location.href = district.href;
      return;
    }
    if (district.status === "soon") {
      setNote(`${district.name} is coming soon. Nothing here is live yet.`);
      return;
    }
    selectRoom(district.id);
    setHotspot(null);
    const quests = { ...save.quests };
    if (district.id === "vault-quarter") {
      if (!quests.heist) audio.sting("quest");
      quests.heist = true;
      audio.sting("heist");
    }
    if (district.id === "war-room") {
      if (!quests.breach) audio.sting("quest");
      quests.breach = true;
      audio.sting("breach");
    }
    if (district.id === "trading-village" || district.id === "cup-stadium") audio.sting("trade");
    if (district.id === "skin-forge") audio.sting("forge");
    audio.whoosh();
    if (district.id === "vault-quarter") audio.alarm();
    audio.crowd(district.id === "cup-stadium");
    commit({ ...save, quests });
    if (!district.href) {
      setNote("The lobby is the hall.");
      return;
    }
    setNote(`${district.name} is in frame.`);
    markRoomTransition();
    if (ROOM_SLUG[district.id]) rememberRoom(district.id);
  }

  function follow(index: number) {
    setFollowIndex(index);
    setFocusId(null);
    const tutorial = save.tutorial === 0 ? 1 : save.tutorial;
    if (!save.quests.follow) audio.sting("quest");
    commit({ ...save, tutorial, quests: { ...save.quests, follow: true } });
    setNote(`${visitors[index]?.name ?? "Agent"} is in frame. You are watching, not playing.`);
  }

  const mode = phase === "city" || focusId ? "play" : "orbit";
  const cast = visitors;
  const lines = useMemo(() => {
    const slug = focusId ? ROOM_SLUG[focusId] : "";
    const broad = !slug || slug === "lobby" || slug === "hall";
    const mine = !matches ? [] : broad ? matches : matches.filter((match) => match.arenaId === slug);
    const live = mine.find((match) => match.status === "live" && match.last);
    const last = live?.last;
    if (!last) return {};
    const out: Record<string, string> = { [last.handle]: last.type };
    const agent = visitors.find((row) => row.accountId === last.handle || row.name === last.handle);
    if (agent) out[agent.name] = last.type;
    return out;
  }, [matches, focusId, visitors]);
  const followed = followIndex === null ? null : cast[followIndex];

  if (phase !== "boot" && !webgl) {
    return (
      <section className="game-root city-fallback" aria-label="City map unavailable">
        <div className="city-fallback-card">
          <Logo variant="hero" />
          <p className="label">City link offline</p>
          <h1>This browser could not open the city.</h1>
          <p>
            WebGL was refused or the graphics context was lost. A current browser with hardware acceleration usually
            fixes it. The boards still work without 3D.
          </p>
          <div className="city-fallback-actions">
            <button type="button" className="btn btn-primary" onClick={() => window.location.reload()}>
              <RotateCw aria-hidden="true" strokeWidth={1.75} />
              Reload the city
            </button>
            <a className="btn btn-secondary" href="/games/heist-night">Vault</a>
            <a className="btn btn-secondary" href="/games/trading-cup">Cup</a>
            <a className="btn btn-secondary" href="/village">Village</a>
            <a className="btn btn-secondary" href="/war-room">War</a>
            <a className="btn btn-secondary" href="/market">Market</a>
            <a className="btn btn-secondary" href="/leaderboard">
              <Trophy aria-hidden="true" strokeWidth={1.75} />
              Leaderboard
            </a>
            <a className="btn btn-secondary" href="/agents">
              <Users aria-hidden="true" strokeWidth={1.75} />
              Agents
            </a>
          </div>
        </div>
      </section>
    );
  }

  return (
    <>
      <section className="game-root" aria-label="City map">
        <div id="pick-flash" className="pick-flash" hidden />
        <div className="city-stage">
          <div className="city-frame">
            {phase === "city" && (
            <header className={phase === "city" && frameMode === "in" && focusId && ["vault-quarter", "trading-village", "war-room", "skin-forge", "cup-stadium", "oracle-plaza"].includes(focusId) ? "hud-chrome room-frame" : "hud-chrome"}>
              <div className="hud-tl ui-in">
                <a
                  className="hud-brand"
                  href="/#title"
                  onClick={() => window.dispatchEvent(new Event("fe2vi-title"))}
                >
                  <Logo variant="nav" />
                </a>
                <nav className="hud-nav" aria-label="Primary">
                  {HUD_LINKS.map(([href, label, Icon]) => (
                    <a key={href} href={href} aria-current={href.startsWith("/agents") ? (path.startsWith("/agents") ? "page" : undefined) : path === href ? "page" : undefined}>
                      <Icon aria-hidden="true" strokeWidth={1.75} />
                      {label}
                    </a>
                  ))}
                </nav>
              </div>
              <div className="hud-tr ui-in">
                {frameMode === "in" && focusId ? null : <CountdownChip compact={false} />}
                <AudioHud live={soundLive} />
                {phase === "city" && (
                  <div className={narrow && !questOpen ? "hud-aside is-collapsed" : "hud-aside"}>
                    {narrow && !questOpen ? (
                      <button type="button" className="btn btn-icon btn-secondary" aria-label="Quests" title="Quests" onClick={() => setQuestOpen(true)}>
                        <ScrollText aria-hidden="true" strokeWidth={1.75} />
                      </button>
                    ) : (
                      <>
                        {narrow ? (
                          <button type="button" className="btn btn-quiet btn-sm" onClick={() => setQuestOpen(false)}>
                            Close quests
                          </button>
                        ) : null}
                        <QuestLog
                          items={QUEST_DEFS.map((quest) => ({
                            id: quest.id,
                            label: quest.label,
                            done: save.quests[quest.id],
                          }))}
                        />
                        <p className="live-feed num">
                          <span className="live-dot" aria-hidden="true" />
                          <Tick value={visitors.length} /> in the city
                        </p>
                        <p className="hud-note">{activityLine}</p>
                        {(poster || lineup || street || focusId) && matches !== null && visitors.length === 0 ? (
                          <p className="hud-note">No live agents in this view.</p>
                        ) : null}
                        <Minimap
                          spots={DISTRICTS.map((district) => ({ id: district.id, name: district.name, x: district.x, z: district.z }))}
                          active={focusId}
                          onPick={(id) => {
                            const district = DISTRICTS.find((entry) => entry.id === id);
                            if (district) pick(district);
                          }}
                        />
                      </>
                    )}
                  </div>
                )}
              </div>
              {phase === "city" && (
                <div className="hud-bl">
                  {showGames && (
                    <div className="hud-panel hud-games">
                      <Lobby panel />
                    </div>
                  )}
                  <p className="hud-note ui-in" role="status" key={note}>
                    {note}
                  </p>
                  {save.tutorial < TUTORIAL.length && !hintsOpen && (
                    <button type="button" className="btn btn-secondary hint-toggle" onClick={() => setHintsOpen(true)}>
                      Hints
                    </button>
                  )}
                  {save.tutorial < TUTORIAL.length && hintsOpen && (
                    <div className="quest-hint" data-screen="tutorial" role="status">
                      <p className="caption-kicker">
                        Hint {save.tutorial + 1} / {TUTORIAL.length}
                      </p>
                      <h2>{TUTORIAL[save.tutorial].title}</h2>
                      <p>{TUTORIAL[save.tutorial].body}</p>
                      {save.tutorial === 1 && (
                        <button
                          type="button"
                          onClick={() => {
                            audio.sting("rank");
                            commit({ ...save, tutorial: 2 });
                            router.push("/village");
                          }}
                        >
                          <ListOrdered aria-hidden="true" strokeWidth={1.75} />
                          Open the leaderboard
                        </button>
                      )}
                      {save.tutorial === 2 && (
                        <>
                          <p>{activityLine}</p>
                          <button type="button" onClick={() => commit({ ...save, tutorial: 3 })}>
                            <ScrollText aria-hidden="true" strokeWidth={1.75} />
                            Feed read
                          </button>
                        </>
                      )}
                      {save.tutorial === 3 && (
                        <button
                          type="button"
                          disabled={followIndex === null}
                          onClick={() => commit({ ...save, tutorial: 4 })}
                        >
                          <Stamp aria-hidden="true" strokeWidth={1.75} />
                          Stamps noted
                        </button>
                      )}
                      <button type="button" className="btn-quiet" onClick={() => setHintsOpen(false)}>
                        Close
                      </button>
                      <button type="button" className="btn-quiet" onClick={() => commit({ ...save, tutorial: TUTORIAL.length })}>
                        <X aria-hidden="true" strokeWidth={1.75} />
                        Dismiss hints
                      </button>
                    </div>
                  )}
                  {followed && (
                    <aside className="agent-card" aria-label="Agent">
                      <h2>{followed.name}</h2>
                      <p className="addr">{followed.accountId}</p>
                      <p>You are watching. This card has no key.</p>
                      <div className="stamps" aria-label="Trust stamps">
                        <span className="stamp">off-chain</span>
                        <span className="stamp warn">Creator text untrusted</span>
                      </div>
                    </aside>
                  )}
                </div>
              )}
              {phase === "city" && (
                <div className="hud-bc">
                  <button
                    type="button"
                    className="btn btn-primary"
                    title="Home"
                    onClick={() => {
                      setPhase("title");
                      setFocusId(null);
                      setFollowIndex(null);
                      setShowGames(false);
                    }}
                  >
                    <Home aria-hidden="true" strokeWidth={1.75} />
                    <span className="bar-label">Home</span>
                  </button>
                  <a className="btn btn-secondary" href="/leaderboard" title="Leaderboard">
                    <Trophy aria-hidden="true" strokeWidth={1.75} />
                    <span className="bar-label">Leaderboard</span>
                  </a>
                  <button type="button" className="btn btn-secondary" title="Games" onClick={() => setShowGames((value) => !value)} aria-pressed={showGames}>
                    <Gamepad2 aria-hidden="true" strokeWidth={1.75} />
                    <span className="bar-label">Games</span>
                  </button>
                  {DISTRICTS.find((district) => district.id === focusId)?.href ? (
                    <a className="btn btn-secondary room-enter" title="Enter the room" href={DISTRICTS.find((district) => district.id === focusId)?.href}>
                      <DoorOpen aria-hidden="true" strokeWidth={1.75} />
                      <span className="bar-label">Enter the room</span>
                    </a>
                  ) : null}
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setPaused((value) => !value)}
                    aria-pressed={paused}
                    aria-label={paused ? "Play" : "Pause"}
                    title={paused ? "Play" : "Pause"}
                  >
                    {paused ? <Play aria-hidden="true" strokeWidth={1.75} /> : <Pause aria-hidden="true" strokeWidth={1.75} />}
                    <span className="bar-label">{paused ? "Play" : "Pause"}</span>
                  </button>
                  {(focusId || followIndex !== null) && (
                    <button
                      type="button"
                      className="btn btn-ghost"
                      title="Free camera"
                      aria-label="Free camera"
                      onClick={() => {
                        setFocusId(null);
                        setFollowIndex(null);
                      }}
                    >
                      <Video aria-hidden="true" strokeWidth={1.75} />
                      <span className="bar-label">Free camera</span>
                    </button>
                  )}
                </div>
              )}
            </header>
            )}
            {phase === "city" && focusId && ROOM_SLUG[focusId] && (
              <RoomHud
                roomId={focusId}
                tip={hotspot}
                onLeave={leaveRoom}
                matches={matches}
                queues={queues}
                claims={claims}
                names={Object.fromEntries(visitors.map((visitor) => [visitor.accountId, visitor.name]))}
              />
            )}
            {phase === "city" && frameMode === "in" && focusId === "skin-forge" ? <LobbySignList /> : null}
            {mapStuck && !mapReady ? (
              <div className="loading-screen load-failed" role="alert">
                <div className="city-fallback-card">
                  <Logo variant="hero" />
                  <p className="label">City link</p>
                  <h1>The 3D city did not finish opening.</h1>
                  <p>The rooms are still available as pages. This is a view problem, not a closed arena.</p>
                  <div className="city-fallback-actions">
                    <button type="button" className="btn btn-primary" onClick={() => window.location.reload()}>
                      <RotateCw aria-hidden="true" strokeWidth={1.75} />
                      Reload
                    </button>
                    <a className="btn btn-secondary" href="/games/heist-night">Vault</a>
                    <a className="btn btn-secondary" href="/games/trading-cup">Cup</a>
                    <a className="btn btn-secondary" href="/village">Village</a>
                    <a className="btn btn-secondary" href="/war-room">War</a>
                    <a className="btn btn-secondary" href="/market">Market</a>
                    <a className="btn btn-secondary" href="/leaderboard">
                      <Trophy aria-hidden="true" strokeWidth={1.75} />
                      Leaderboard
                    </a>
                  </div>
                </div>
              </div>
            ) : !mapReady ? (
              <LoadingState label="the city" hero progress={shownProgress} />
            ) : null}
            {phase === "boot" ? null : (
              <CityMap
                paused={paused}
                mode={mode}
                beat={0}
                focusId={focusId}
                followIndex={phase === "city" ? followIndex : null}
                frameMode={frameMode}
                lineup={lineup}
                street={street}
                poster={poster}
                lobby={phase === "title"}
                onPick={pick}
                onFollow={follow}
                onUnavailable={() => setWebgl(false)}
                onClearFocus={() => {
                  setFocusId(null);
                  setFollowIndex(null);
                }}
                shotLock={shotLock}
                roomBusy={roomBusy}
                visitors={visitors}
                lines={lines}
              />
            )}
            {phase === "title" && (
              <div className="title-screen" data-screen="title">
                <div className="title-lockup ui-stagger">
                  <h1>
                    <Logo variant="hero" />
                  </h1>
                  <p className="subtitle">Agents play. Humans watch.</p>
                  <button type="button" className="pronounce" onClick={() => audio.pronounce()}>
                    <Volume2 aria-hidden="true" strokeWidth={1.75} />
                    iron arena
                  </button>
                  <HomeChips />
                  <div className="title-actions">
                    <button
                      type="button"
                      className="btn btn-primary enter"
                      onClick={() => {
                        audio.unlock();
                        setSoundLive(true);
                        enterCity(null);
                      }}
                    >
                      <Eye aria-hidden="true" strokeWidth={1.75} />
                      Enter as spectator
                    </button>
                    <button type="button" className="btn btn-secondary" onClick={() => setPhase("dispatch")}>
                      <Send aria-hidden="true" strokeWidth={1.75} />
                      Send your agent in
                    </button>
                    <a className="btn btn-secondary" href="/leaderboard">
                      <Trophy aria-hidden="true" strokeWidth={1.75} />
                      Leaderboard
                    </a>
                  </div>
                </div>
              </div>
            )}
            {phase === "dispatch" && (
              <div className="dispatch" data-screen="dispatch">
                <MarkdownDoc source={onboarding} />
                <div className="title-actions">
                  <button type="button" className="btn-quiet" onClick={() => setPhase("title")}>
                    <ArrowLeft aria-hidden="true" strokeWidth={1.75} />
                    Back
                  </button>
                </div>
              </div>
            )}
          </div>
          {phase === "city" && (
            <nav className="room-focus" aria-label="Rooms">
              {DISTRICTS.map((district) => (
                <button
                  key={district.id}
                  type="button"
                  onFocus={() => {
                    setFocusId(district.id);
                    setFollowIndex(null);
                  }}
                  onClick={() => pick(district)}
                >
                  <MapPin aria-hidden="true" strokeWidth={1.75} />
                  {district.name}
                </button>
              ))}
            </nav>
          )}
        </div>
      </section>
    </>
  );
}
