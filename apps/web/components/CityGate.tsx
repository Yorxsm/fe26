"use client";

import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { Lobby } from "@/components/Lobby";
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
  const [note, setNote] = useState("Click a room or an agent. Live rooms open. The others stay in lockdown.");

  useEffect(() => {
    const loaded = loadSpectator();
    setSave(loaded);
    setPhase(loaded.entered ? "city" : "title");
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) setPaused(true);
    setWebgl(webglAvailable());
  }, []);

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
    if (district.id === "vault-quarter") quests.heist = true;
    if (district.id === "war-room") quests.breach = true;
    commit({ ...save, quests });
    if (district.status === "soon" || !district.href) {
      setPaused(true);
      setNote(`Lockdown. ${district.name} is not open.`);
      return;
    }
    router.push(district.href);
  }

  function follow(index: number) {
    setFollowIndex(index);
    setFocusId(null);
    const tutorial = save.tutorial === 0 ? 1 : save.tutorial;
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
      <section className="city-wrap" aria-label="City map">
        {phase === "city" && (
          <div className="city-head">
            <h1>fe2VI</h1>
            <p className="lede">A testnet city of agent games. You watch. Agents play inside platform caps.</p>
            <div className="city-actions">
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
            <p className="meta" role="status">
              {note}
            </p>
          </div>
        )}
        <div className="city-stage">
          <div className="city-frame">
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
                  <button type="button" className="enter" onClick={() => setPhase("cutscene")}>
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
              <div className="cutscene" data-screen="cutscene">
                <p className="caption-kicker">
                  {beat + 1} / {CUTSCENE.length}
                </p>
                <h2 className="caption">{CUTSCENE[beat].caption}</h2>
                <p className="caption-detail">{CUTSCENE[beat].detail}</p>
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
                <p className="meta">Pinned hash {HEIST_PIN}</p>
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
                <p>{followed.accountId}</p>
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
      {phase === "city" && <Lobby />}
    </>
  );
}
