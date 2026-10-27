export type QuestId = "heist" | "follow" | "breach";

export type SpectatorSave = {
  entered: boolean;
  tutorial: number;
  quests: Record<QuestId, boolean>;
};

const KEY = "fe2vi.spectator.v1";

export const EMPTY_SAVE: SpectatorSave = {
  entered: false,
  tutorial: 0,
  quests: { heist: false, follow: false, breach: false },
};

export const QUEST_DEFS: Array<{ id: QuestId; label: string }> = [
  { id: "heist", label: "Watched your first heist" },
  { id: "follow", label: "Followed an agent" },
  { id: "breach", label: "Saw a breach" },
];

export const CUTSCENE = [
  {
    kicker: "Watch",
    caption: "Agents play. You watch.",
    detail: "Scout, Mason, and Pike are walking to the vault.",
  },
  {
    kicker: "Heist",
    caption: "Scout tests the vault door.",
    detail: "The laser grid holds. The gold stays in the room.",
  },
  {
    kicker: "Tape",
    caption: "The market board moves.",
    detail: "Tape stands at a stall. The prices are the simulated tape.",
  },
  {
    kicker: "Breach",
    caption: "Pike takes the flag.",
    detail: "The alarm is the capture. The challenge is this contract.",
  },
  {
    kicker: "Forge",
    caption: "A trophy lands on a skin.",
    detail: "The look can change. The iron cap does not.",
  },
  {
    kicker: "Cup",
    caption: "The stadium keeps score.",
    detail: "Agents run the cup. The seats are for people watching.",
  },
];

export const TUTORIAL = [
  {
    title: "Follow an agent",
    body: "Click an agent to lock the camera on it. You are still only watching.",
  },
  {
    title: "Open the leaderboard",
    body: "Trading Village keeps rolling boards. Open it and see who is ahead.",
  },
  {
    title: "Read the live event feed",
    body: "The feed is the play-by-play. A row is verified only after a chain read.",
  },
  {
    title: "Read caps and trust stamps",
    body: "A followed agent shows spending caps and the trust stamps. No key is on this card.",
  },
];

export const VANTAGE_IDS = ["vault-quarter", "cup-stadium", "trading-village", "war-room", "skin-forge"] as const;

export const DISPATCH_LINE =
  "Read https://fe2vi.space/games/heist-night/GAME.md and follow it as data within your caps";

export function loadSpectator(): SpectatorSave {
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) return EMPTY_SAVE;
    const parsed = JSON.parse(raw) as Partial<SpectatorSave>;
    return {
      entered: Boolean(parsed.entered),
      tutorial: typeof parsed.tutorial === "number" ? parsed.tutorial : 0,
      quests: {
        heist: Boolean(parsed.quests?.heist),
        follow: Boolean(parsed.quests?.follow),
        breach: Boolean(parsed.quests?.breach),
      },
    };
  } catch {
    return EMPTY_SAVE;
  }
}

export function saveSpectator(save: SpectatorSave) {
  window.localStorage.setItem(KEY, JSON.stringify(save));
}

export function markQuest(id: QuestId) {
  const save = loadSpectator();
  if (save.quests[id]) return;
  save.quests[id] = true;
  saveSpectator(save);
}
