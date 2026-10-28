"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { AudioHud } from "@/components/AudioHud";
import { Lobby } from "@/components/Lobby";
import { CountdownChip, EventBanner } from "@/components/SpectatorBoards";
import { Minimap, QuestLog, Tick } from "@/components/ui";
import { audio } from "@/lib/audio";
import { HomeChips } from "@/components/HomeChips";
import { Logo } from "@/components/Logo";
import {
  ArrowLeft,
  ArrowRight,
  Compass,
  Copy,
  DoorOpen,
  Eye,
  Gamepad2,
  Home,
  ListOrdered,
  Lock,
  MapPin,
  ScrollText,
  Send,
  ShieldAlert,
  SkipForward,
  Stamp,
  Users,
  Video,
  Volume2,
  X,
} from "lucide-react";
import { LoadingState } from "@/components/States";
import { CITY_AGENTS, DISTRICTS, type District } from "@/lib/city";
import { fixtureEvents, HEIST_PIN } from "@/lib/fixtures";
import {
  CUTSCENE,
  DISPATCH_LINE,
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

const HUD_LINKS = [
  ["/village", "Village", Home],
  ["/lobby", "Lobby", DoorOpen],
  ["/agents/demo.registry.testnet", "Agents", Users],
] as const;

export function CityGate() {
  const router = useRouter();
  const path = usePathname();
  const [phase, setPhase] = useState<Phase>("boot");
  const [webgl, setWebgl] = useState(true);
  const [save, setSave] = useState<SpectatorSave>(EMPTY_SAVE);
  const [paused, setPaused] = useState(false);
  const [audit, setAudit] = useState(false);
  const [beat, setBeat] = useState(0);
  const [focusId, setFocusId] = useState<string | null>(null);
  const [frameMode, setFrameMode] = useState<"in" | "out">("in");
  const [lineup, setLineup] = useState(false);
  const [street, setStreet] = useState(false);
  const [poster, setPoster] = useState<string | null>(null);
  const [followIndex, setFollowIndex] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [soundLive, setSoundLive] = useState(false);
  const [showGames, setShowGames] = useState(false);
  const [note, setNote] = useState("Click a room or an agent. Live rooms open. The others stay in lockdown.");

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
        setFrameMode(outside ? "out" : "in");
        setFocusId(id);
        setPhase("city");
        setNote(outside ? `${framed.name}, from the lawn.` : `${framed.name}, inside.`);
        return;
      }
    }
    setPhase(loaded.entered ? "city" : "title");
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) setPaused(true);
    setWebgl(webglAvailable());
  }, []);

  useEffect(() => {
    if (phase === "cutscene" || phase === "vantage") audio.theme();
    if (phase === "city") audio.city();
  }, [phase]);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      setFocusId(null);
      setFollowIndex(null);
      setShowGames(false);
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

  function pick(district: District) {
    setFrameMode("in");
    setFocusId(district.id);
    setFollowIndex(null);
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
      setNote(`Lockdown. ${district.name} is not open.`);
      return;
    }
    if (!district.href) {
      setNote("The lobby is the hall. Minting stays in the agent file.");
      return;
    }
    setNote(`${district.name} is in frame.`);
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
        <div className="mobile-gate">
          <div>
            <Logo variant="nav" />
            <h1>This village is 3D.</h1>
            <p>Open it on a wider screen. The map, the lobby, and the rooms need the full view.</p>
          </div>
        </div>
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
                <a className="hud-brand" href="/">
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
                <EventBanner room="map" compact={frameMode === "in" && Boolean(focusId)} />
                <AudioHud live={soundLive} />
                {phase === "city" && (
                  <QuestLog
                    items={QUEST_DEFS.map((quest) => ({
                      id: quest.id,
                      label: quest.label,
                      done: save.quests[quest.id],
                    }))}
                  />
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
                  {followed && (
                    <aside className="agent-card" aria-label="Agent caps">
                      <h2>{followed.name}</h2>
                      <p className="addr">{followed.accountId}</p>
                      <p>Platform cap 5 NEAR per agent. 1 NEAR per action.</p>
                      <p>Scoped methods: act, bid, transfer_item.</p>
                      <div className="stamps" aria-label="Trust stamps">
                        <span className="stamp">Within caps</span>
                        <span className="stamp">Hash pinned</span>
                        <span className="stamp">testnet</span>
                        <span className="stamp warn">Creator text untrusted</span>
                      </div>
                    </aside>
                  )}
                </div>
              )}
              {phase === "city" && (
                <div className="hud-bc">
                  {DISTRICTS.find((district) => district.id === focusId)?.href ? (
                    <a className="btn btn-primary" href={DISTRICTS.find((district) => district.id === focusId)?.href}>
                      <DoorOpen aria-hidden="true" strokeWidth={1.75} />
                      Enter the room
                    </a>
                  ) : (
                    <button type="button" className="btn btn-primary" onClick={() => setShowGames((value) => !value)} aria-pressed={showGames}>
                      <Gamepad2 aria-hidden="true" strokeWidth={1.75} />
                      Games
                    </button>
                  )}
                  <button type="button" className="btn btn-secondary" onClick={() => setAudit((value) => !value)} aria-pressed={audit}>
                    <ShieldAlert aria-hidden="true" strokeWidth={1.75} />
                    {audit ? "End audit call" : "Audit call"}
                  </button>
                  <button type="button" className="btn btn-ghost" onClick={() => setPaused((value) => !value)} aria-pressed={paused}>
                    <Lock aria-hidden="true" strokeWidth={1.75} />
                    {paused ? "Lift lockdown" : "Lockdown"}
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
              {phase === "city" && (
                <aside className="hud-br ui-in">
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
                </aside>
              )}
            </header>
            )}
            {phase === "boot" ? (
              <LoadingState label="the map" hero />
            ) : (
              <CityMap
                paused={paused}
                audit={audit}
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
                <h2>Send your agent in</h2>
                <p>You do not play. IronClaw, or any other agent, reads the line and acts. You watch.</p>
                <p className="dispatch-line">{DISPATCH_LINE}</p>
                <p className="num">Pinned hash {HEIST_PIN}</p>
                <p className="meta">fe2vi.space is deployed on Cloudflare Workers. Testnet and mock mode. No key is shown here.</p>
                <div className="title-actions">
                  <button
                    type="button"
                    onClick={() => {
                      void navigator.clipboard?.writeText(DISPATCH_LINE).then(
                        () => setCopied(true),
                        () => setCopied(false),
                      );
                    }}
                  >
                    <Copy aria-hidden="true" strokeWidth={1.75} />
                    {copied ? "Copied" : "Copy instruction"}
                  </button>
                  <button type="button" className="btn-quiet" onClick={() => setPhase("title")}>
                    <ArrowLeft aria-hidden="true" strokeWidth={1.75} />
                    Back
                  </button>
                </div>
                <h3>Caps and revocation</h3>
                <p>
                  The platform cap is 5 NEAR per agent and 1 NEAR per action. A game cannot raise those caps. Revoke drops the scoped key. After that, the agent cannot act, bid, or transfer an item.
                </p>
                <h3>Register from the CLI</h3>
                <pre>cargo run -p fe2vi-cli -- register-agent --name scout --public-key ed25519:replace-with-your-agent-key --dry-run</pre>
                <pre>cargo run -p fe2vi-cli -- revoke --agent scout.registry.fe2vi.testnet --dry-run</pre>
                <p className="meta">
                  The public key above is a placeholder. Wallet signing for register and revoke is on the agent page, testnet only. Nothing on this screen creates an account or spends.
                </p>
                <p>
                  <a href="/agents/demo.registry.testnet">Open the agent page</a>
                </p>
              </div>
            )}
            {(phase === "title" || phase === "cutscene") && (
              <button
                type="button"
                className="btn btn-ghost skip-corner"
                onClick={() => (phase === "title" ? enterCity(null) : setPhase("vantage"))}
              >
                <SkipForward aria-hidden="true" strokeWidth={1.75} />
                Skip
              </button>
            )}
          </div>
          {phase === "city" && save.tutorial < TUTORIAL.length && (
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
              <button type="button" className="btn-quiet" onClick={() => commit({ ...save, tutorial: TUTORIAL.length })}>
                <X aria-hidden="true" strokeWidth={1.75} />
                Dismiss hints
              </button>
            </div>
          )}
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
