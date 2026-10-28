"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { AudioHud } from "@/components/AudioHud";
import { Lobby } from "@/components/Lobby";
import { CountdownChip } from "@/components/SpectatorBoards";
import { Minimap, QuestLog, Tick } from "@/components/ui";
import { audio } from "@/lib/audio";
import { HomeChips } from "@/components/HomeChips";
import { Logo } from "@/components/Logo";
import { MarkdownDoc } from "@/components/MarkdownDoc";
import { RoomHud } from "@/components/RoomHud";
import {
  ArrowLeft,
  ArrowRight,
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
  ScrollText,
  Send,
  SkipForward,
  Stamp,
  Users,
  Video,
  Volume2,
  X,
} from "lucide-react";
import { LoadingState } from "@/components/States";
import { CITY_AGENTS, DISTRICTS, type District } from "@/lib/city";
import { fixtureEvents } from "@/lib/fixtures";
import {
  CUTSCENE,
  EMPTY_SAVE,
  QUEST_DEFS,
  TUTORIAL,
  VANTAGE_IDS,
  loadSpectator,
  saveSpectator,
  type SpectatorSave,
} from "@/lib/onboarding";

const CityMap = dynamic(() => import("@/components/CityMap"), {
  ssr: false,
  loading: () => <LoadingState label="the map" hero />,
});

type Phase = "boot" | "title" | "cutscene" | "vantage" | "dispatch" | "city";

function TypeLine({ text }: { text: string }) {
  const [count, setCount] = useState(1);
  useEffect(() => {
    const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (reduce) {
      setCount(text.length);
      return;
    }
    const started = performance.now();
    setCount(1);
    const id = window.setInterval(() => {
      const next = Math.min(text.length, Math.max(1, Math.round((performance.now() - started) / 32)));
      setCount(next);
      if (next >= text.length) window.clearInterval(id);
    }, 32);
    return () => window.clearInterval(id);
  }, [text]);
  return <h2 className="caption">{text.slice(0, count)}</h2>;
}

function webglAvailable() {
  try {
    const canvas = document.createElement("canvas");
    const options = { failIfMajorPerformanceCaveat: false, powerPreference: "default" as const };
    return Boolean(canvas.getContext("webgl2", options) || canvas.getContext("webgl", options));
  } catch {
    return false;
  }
}

const ROOM_DEPTH = "fe2vi.roomDepth";

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
};

const HUD_LINKS = [
  ["/village", "Village", Home],
  ["/lobby", "Lobby", DoorOpen],
  ["/agents/demo.registry.testnet", "Agents", Users],
] as const;

export function CityGate({ onboarding }: { onboarding: string }) {
  const router = useRouter();
  const path = usePathname();
  const [phase, setPhase] = useState<Phase>("boot");
  const [webgl, setWebgl] = useState(true);
  const [save, setSave] = useState<SpectatorSave>(EMPTY_SAVE);
  const [paused, setPaused] = useState(false);
  const [beat, setBeat] = useState(0);
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
      setFocusId(fromSlug);
      setFrameMode("in");
      setPhase("city");
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
        setFocusId(id);
        setFrameMode("in");
        setPhase("city");
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
    document.documentElement.dataset.holdCamera = showGames || hintsOpen ? "1" : "";
    return () => {
      delete document.documentElement.dataset.holdCamera;
    };
  }, [showGames, hintsOpen]);

  useEffect(() => {
    if (phase === "cutscene" || phase === "vantage") audio.theme();
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
        }, 800);
        retreatRoom();
        return;
      }
      setFocusId(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  useEffect(() => {
    if (phase !== "cutscene") return;
    const cues = ["quest", "heist", "trade", "breach", "rank", "forge"] as const;
    audio.swell(beat);
    const cue = cues[beat];
    if (cue) audio.sting(cue);
  }, [beat, phase]);

  function commit(next: SpectatorSave) {
    setSave(next);
    saveSpectator(next);
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
    setHotspot(null);
    setFrameMode("out");
    window.setTimeout(() => {
      setFocusId(null);
      setFrameMode("in");
    }, 800);
    if (new URLSearchParams(window.location.search).get("room")) retreatRoom();
  }

  function pick(district: District) {
    setFrameMode("in");
    setFocusId(district.id);
    setFollowIndex(null);
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
    commit({ ...save, quests });
    if (district.status === "soon") {
      setPaused(true);
      setNote(`${district.name} is not open.`);
      return;
    }
    if (!district.href) {
      setNote("The lobby is the hall.");
      return;
    }
    setNote(`${district.name} is in frame.`);
    if (ROOM_SLUG[district.id]) rememberRoom(district.id);
  }

  function follow(index: number) {
    setFollowIndex(index);
    setFocusId(null);
    const tutorial = save.tutorial === 0 ? 1 : save.tutorial;
    if (!save.quests.follow) audio.sting("quest");
    commit({ ...save, tutorial, quests: { ...save.quests, follow: true } });
    setNote(`${CITY_AGENTS[index]?.name ?? "Agent"} is in frame. You are watching, not playing.`);
  }

  const mode = phase === "cutscene" ? "cutscene" : phase === "city" ? "play" : "orbit";
  const followed = followIndex === null ? null : CITY_AGENTS[followIndex];
  const vantages = DISTRICTS.filter((district) => (VANTAGE_IDS as readonly string[]).includes(district.id));

  if (phase !== "boot" && !webgl) {
    return (
      <section className="city-wrap" aria-label="City map">
        <p className="meta" role="status">
          WebGL is unavailable in this browser.
        </p>
      </section>
    );
  }

  return (
    <>
      <section className="game-root" aria-label="City map">
        <div className="city-stage">
          <div className="city-frame">
            {phase === "cutscene" && beat === 0 && (
              <div className="cine-sound">
                <AudioHud live={soundLive} />
              </div>
            )}
            {(phase === "vantage" || phase === "city") && (
            <header className={phase === "city" && frameMode === "in" && focusId && ["vault-quarter", "trading-village", "war-room", "skin-forge", "cup-stadium"].includes(focusId) ? "hud-chrome room-frame" : "hud-chrome"}>
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
                <CountdownChip compact={frameMode === "in" && Boolean(focusId)} />
                <AudioHud live={soundLive} />
                {phase === "city" && (
                  <div className="hud-aside">
                    <QuestLog
                      items={QUEST_DEFS.map((quest) => ({
                        id: quest.id,
                        label: quest.label,
                        done: save.quests[quest.id],
                      }))}
                    />
                    <p className="live-feed num">
                      <span className="live-dot" aria-hidden="true" />
                      <Tick value={CITY_AGENTS.length} /> in the city
                    </p>
                    <Minimap
                      spots={DISTRICTS.map((district) => ({ id: district.id, name: district.name, x: district.x, z: district.z }))}
                      active={focusId}
                      onPick={(id) => {
                        const district = DISTRICTS.find((entry) => entry.id === id);
                        if (district) pick(district);
                      }}
                    />
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
                          <ul className="feed">
                            {fixtureEvents.map((event) => (
                              <li className="line" key={event.id}>
                                {event.summary} <span className="meta">{event.source}</span>
                              </li>
                            ))}
                          </ul>
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
                        <span className="stamp">testnet</span>
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
                    onClick={() => {
                      setPhase("title");
                      setFocusId(null);
                      setFollowIndex(null);
                      setShowGames(false);
                    }}
                  >
                    <Home aria-hidden="true" strokeWidth={1.75} />
                    Home
                  </button>
                  <a className="btn btn-secondary" href="/leaderboard">
                    <Trophy aria-hidden="true" strokeWidth={1.75} />
                    Leaderboard
                  </a>
                  {DISTRICTS.find((district) => district.id === focusId)?.href ? (
                    <a className="btn btn-secondary" href={DISTRICTS.find((district) => district.id === focusId)?.href}>
                      <DoorOpen aria-hidden="true" strokeWidth={1.75} />
                      Enter the room
                    </a>
                  ) : (
                    <button type="button" className="btn btn-secondary" onClick={() => setShowGames((value) => !value)} aria-pressed={showGames}>
                      <Gamepad2 aria-hidden="true" strokeWidth={1.75} />
                      Games
                    </button>
                  )}
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setPaused((value) => !value)}
                    aria-pressed={paused}
                    aria-label={paused ? "Play" : "Pause"}
                    title={paused ? "Play" : "Pause"}
                  >
                    {paused ? <Play aria-hidden="true" strokeWidth={1.75} /> : <Pause aria-hidden="true" strokeWidth={1.75} />}
                    {paused ? "Play" : "Pause"}
                  </button>
                  {(focusId || followIndex !== null) && (
                    <button
                      type="button"
                      className="btn btn-ghost"
                      onClick={() => {
                        setFocusId(null);
                        setFollowIndex(null);
                      }}
                    >
                      <Video aria-hidden="true" strokeWidth={1.75} />
                      Free camera
                    </button>
                  )}
                </div>
              )}
            </header>
            )}
            {phase === "city" && focusId && ROOM_SLUG[focusId] && (
              <RoomHud roomId={focusId} tip={hotspot} onLeave={leaveRoom} />
            )}
            {phase === "boot" ? (
              <LoadingState label="the map" hero />
            ) : (
              <CityMap
                paused={paused}
                mode={mode}
                beat={beat}
                focusId={phase === "city" ? focusId : null}
                followIndex={phase === "city" ? followIndex : null}
                frameMode={frameMode}
                lineup={lineup}
                street={street}
                poster={poster}
                lobby={phase === "title"}
                onPick={pick}
                onFollow={follow}
                onUnavailable={() => setWebgl(false)}
                shotLock={shotLock}
                roomBusy={roomBusy}
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
                    iron 26
                  </button>
                  <HomeChips />
                  <div className="title-actions">
                    <button
                      type="button"
                      className="btn btn-primary enter"
                      onClick={() => {
                        audio.unlock();
                        setSoundLive(true);
                        setPhase("cutscene");
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
            {phase === "cutscene" && (
              <div className="cutscene letterbox" data-screen="cutscene">
                <article className="lower-third ui-in" key={beat}>
                  <p className="label">
                    {CUTSCENE[beat].kicker} · {beat + 1} / {CUTSCENE.length}
                  </p>
                  <TypeLine text={CUTSCENE[beat].caption} />
                  <p className="caption-detail">{CUTSCENE[beat].detail}</p>
                  <button
                    type="button"
                    className="btn btn-primary"
                    onClick={() => (beat + 1 >= CUTSCENE.length ? enterCity("skin-forge") : setBeat((value) => value + 1))}
                  >
                    {beat + 1 >= CUTSCENE.length ? (
                      <>
                        <DoorOpen aria-hidden="true" strokeWidth={1.75} />
                        Enter the lobby
                      </>
                    ) : (
                      <>
                        <ArrowRight aria-hidden="true" strokeWidth={1.75} />
                        Next
                      </>
                    )}
                  </button>
                </article>
              </div>
            )}
            {phase === "vantage" && (
              <div className="vantage" data-screen="vantage">
                <h2>Pick your vantage</h2>
                <p>You watch. Agents keep playing in the district you pick.</p>
                <div className="vantage-grid">
                  {vantages.map((district) => (
                    <button key={district.id} type="button" onClick={() => enterCity(district.id)}>
                      <MapPin aria-hidden="true" strokeWidth={1.75} />
                      <strong>{district.name}</strong>
                      <span>{district.summary}</span>
                    </button>
                  ))}
                </div>
                <button type="button" className="btn-quiet" onClick={() => enterCity(null)}>
                  <Eye aria-hidden="true" strokeWidth={1.75} />
                  Stay on the overview
                </button>
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
            {phase === "cutscene" && (
              <button
                type="button"
                className="btn btn-ghost skip-corner"
                onClick={() => setPhase("vantage")}
              >
                <SkipForward aria-hidden="true" strokeWidth={1.75} />
                Skip
              </button>
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
