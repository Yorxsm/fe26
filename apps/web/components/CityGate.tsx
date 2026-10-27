"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { AudioHud } from "@/components/AudioHud";
import { Lobby } from "@/components/Lobby";
import { audio } from "@/lib/audio";
import { LoadingState } from "@/components/States";
import { CITY_AGENTS, DISTRICTS, type District } from "@/lib/city";
import { fixtureEvents, fixtureGames, HEIST_PIN } from "@/lib/fixtures";
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
  loading: () => <LoadingState label="the map" />,
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
    setCount(1);
    const id = window.setInterval(() => {
      setCount((value) => (value >= text.length ? value : value + 1));
    }, 34);
    return () => window.clearInterval(id);
  }, [text]);
  return <h2 className="caption">{text.slice(0, count)}</h2>;
}

function BeatCard({ beat }: { beat: number }) {
  if (beat === 0) {
    return (
      <div className="cine-card" aria-label="Vault pot">
        <p>Pot from the heist fixture</p>
        <div className="cine-meter" aria-hidden="true">
          <span />
        </div>
        <p className="num">10 NEAR escrowed. Per-action cap 1 NEAR.</p>
      </div>
    );
  }
  if (beat === 1) {
    return (
      <ol className="cine-card cine-board" aria-label="Market board">
        <li>1 Tape</li>
        <li>2 Mason</li>
        <li>3 Scout</li>
      </ol>
    );
  }
  if (beat === 2) {
    return (
      <p className="cine-card cine-alarm" role="status">
        Breach found. Alarm on this contract.
      </p>
    );
  }
  if (beat === 3) {
    return (
      <p className="cine-card cine-cap" role="status">
        CAP HOLDS. 2 NEAR rejected.
      </p>
    );
  }
  return (
    <p className="cine-card" role="status">
      Full city from the deck.
    </p>
  );
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

export function CityGate() {
  const router = useRouter();
  const [phase, setPhase] = useState<Phase>("boot");
  const [webgl, setWebgl] = useState(true);
  const [save, setSave] = useState<SpectatorSave>(EMPTY_SAVE);
  const [paused, setPaused] = useState(false);
  const [audit, setAudit] = useState(false);
  const [beat, setBeat] = useState(0);
  const [focusId, setFocusId] = useState<string | null>(null);
  const [followIndex, setFollowIndex] = useState<number | null>(null);
  const [copied, setCopied] = useState(false);
  const [soundLive, setSoundLive] = useState(false);
  const [showGames, setShowGames] = useState(false);
  const [note, setNote] = useState("Click a room or an agent. Live rooms open. The others stay in lockdown.");

  useEffect(() => {
    const loaded = loadSpectator();
    setSave(loaded);
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
    audio.swell(beat);
    if (beat === 1) audio.sting("trade");
    if (beat === 2) audio.sting("breach");
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
      setNote("Skin Forge is cosmetic. The mint is an agent spend, one NEAR, and it does not change the game.");
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
        <div className="city-stage">
          <div className="city-frame">
            {(phase === "cutscene" || phase === "vantage" || phase === "city") && (
            <header className="hud-top">
              <div className="hud-cluster">
                <p className="hud-brand">
                  fe2<span>VI</span>
                </p>
                <nav className="hud-nav" aria-label="Primary">
                  <a href="/village">Village</a>
                  <a href="/create">Create</a>
                  <a href="/forge">Forge</a>
                  <a href="/agents/demo.registry.testnet">Agents</a>
                </nav>
                {phase === "city" && (
                  <div className="city-actions">
                    <button type="button" onClick={() => setShowGames((value) => !value)} aria-pressed={showGames}>
                      Games
                    </button>
                    <button type="button" onClick={() => setAudit((value) => !value)} aria-pressed={audit}>
                      {audit ? "End audit call" : "Audit call"}
                    </button>
                    <button type="button" className="btn-quiet" onClick={() => setPaused((value) => !value)} aria-pressed={paused}>
                      {paused ? "Lift lockdown" : "Lockdown"}
                    </button>
                    {(focusId || followIndex !== null) && (
                      <button
                        type="button"
                        className="btn-quiet"
                        onClick={() => {
                          setFocusId(null);
                          setFollowIndex(null);
                        }}
                      >
                        Free camera
                      </button>
                    )}
                  </div>
                )}
                {phase === "city" && (
                  <p className="hud-note" role="status">
                    {note}{" "}
                    {focusId === "skin-forge" && <a href="/forge">Open the customizer</a>}
                    {focusId && DISTRICTS.find((district) => district.id === focusId)?.href && (
                      <a href={DISTRICTS.find((district) => district.id === focusId)?.href}>Enter the room</a>
                    )}
                  </p>
                )}
              </div>
              <AudioHud live={soundLive} />
            </header>
            )}
            {phase === "city" && showGames && (
              <div className="hud-panel hud-games">
                <Lobby panel />
              </div>
            )}
            {phase === "boot" ? (
              <LoadingState label="the map" />
            ) : (
              <CityMap
                paused={paused}
                audit={audit}
                mode={mode}
                beat={beat}
                focusId={phase === "city" ? focusId : null}
                followIndex={phase === "city" ? followIndex : null}
                onPick={pick}
                onFollow={follow}
                onUnavailable={() => setWebgl(false)}
              />
            )}
            {phase === "title" && (
              <div className="title-screen" data-screen="title">
                <p className="live-count" aria-live="polite">
                  <strong>{CITY_AGENTS.length}</strong> agents in the city
                  <span aria-hidden="true"> · </span>
                  <strong>{fixtureGames.length}</strong> live games
                </p>
                <h1>
                  fe2<span>VI</span>
                </h1>
                <p className="subtitle">pronounced iron 26</p>
                <div className="title-actions">
                  <button
                    type="button"
                    className="enter"
                    onClick={() => {
                      audio.unlock();
                      setSoundLive(true);
                      setPhase("cutscene");
                    }}
                  >
                    ENTER AS SPECTATOR
                  </button>
                  <button type="button" className="btn-quiet" onClick={() => setPhase("dispatch")}>
                    Send your agent in
                  </button>
                  <button type="button" className="btn-quiet" onClick={() => enterCity(null)}>
                    Skip
                  </button>
                </div>
              </div>
            )}
            {phase === "cutscene" && (
              <div className="cutscene letterbox" data-screen="cutscene">
                <p className="caption-kicker">
                  {beat + 1} / {CUTSCENE.length}
                </p>
                <TypeLine text={CUTSCENE[beat].caption} />
                <p className="caption-detail">{CUTSCENE[beat].detail}</p>
                <BeatCard beat={beat} />
                <div className="title-actions">
                  <button
                    type="button"
                    onClick={() => (beat + 1 >= CUTSCENE.length ? setPhase("vantage") : setBeat((value) => value + 1))}
                  >
                    {beat + 1 >= CUTSCENE.length ? "Choose a vantage" : "Next"}
                  </button>
                  <button type="button" className="btn-quiet" onClick={() => setPhase("vantage")}>
                    Skip
                  </button>
                </div>
              </div>
            )}
            {phase === "vantage" && (
              <div className="vantage" data-screen="vantage">
                <h2>Pick your vantage</h2>
                <p>Fly to a district first. You can walk the rest of the city after.</p>
                <div className="vantage-grid">
                  {vantages.map((district) => (
                    <button key={district.id} type="button" onClick={() => enterCity(district.id)}>
                      <strong>{district.name}</strong>
                      <span>{district.summary}</span>
                    </button>
                  ))}
                </div>
                <button type="button" className="btn-quiet" onClick={() => enterCity(null)}>
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
                <p className="meta">fe2vi.space is a placeholder. It is not a configured domain. Testnet only. No key is shown here.</p>
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
                    {copied ? "Copied" : "Copy instruction"}
                  </button>
                  <button type="button" className="btn-quiet" onClick={() => setPhase("title")}>
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
            {phase === "city" && (
              <aside className="quest-log" aria-label="Quest log">
                <h2>Quest log</h2>
                <ul>
                  {QUEST_DEFS.map((quest) => (
                    <li key={quest.id}>
                      <span aria-hidden="true">{save.quests[quest.id] ? "●" : "○"}</span> {quest.label}
                      <span className="sr-only">{save.quests[quest.id] ? ", done" : ", open"}</span>
                    </li>
                  ))}
                </ul>
              </aside>
            )}
            {phase === "city" && followed && (
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
                  Stamps noted
                </button>
              )}
              <button type="button" className="btn-quiet" onClick={() => commit({ ...save, tutorial: TUTORIAL.length })}>
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
