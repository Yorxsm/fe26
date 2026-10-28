export type BoardSign = "vault" | "cup" | "floor" | "war";

export type FrameBoard = {
  name: string;
  at: [number, number, number];
  size: [number, number];
  sign?: BoardSign;
};

/**
 * Board rectangles in room-local meters. The frame audit projects this same
 * list, so a moved screen has to be updated here or the shot script fails.
 */
export const FRAME_BOARDS: Record<string, FrameBoard[]> = {
  "vault-quarter": [
    { name: "SCHEDULE", at: [-3.6, 1.85, -5.9], size: [2.8, 1.35] },
    { name: "SEASON", at: [3.6, 1.85, -5.9], size: [2.8, 1.35] },
  ],
  "trading-village": [
    { name: "TICKER", at: [0, 4.55, -5.78], size: [11.4, 0.7] },
    { name: "PNL", at: [5.55, 2.35, -5.78], size: [2.2, 1.5] },
  ],
  "war-room": [
    { name: "CONTRACT", at: [-5.3, 2.35, -4.6], size: [1.45, 0.62] },
    { name: "WEB", at: [0, 2.35, -4.6], size: [1.45, 0.62] },
    { name: "CRYPTO", at: [5.3, 2.35, -4.6], size: [1.45, 0.62] },
    { name: "SCORE", at: [0, 2.95, -5.82], size: [6.2, 1.05] },
  ],
  "skin-forge": [
    { name: "HALL", at: [-2.6, 5.15, -5.78], size: [4.6, 1.35] },
    { name: "SEASON", at: [2.7, 5.15, -5.78], size: [4.4, 1.35] },
    { name: "VAULT", at: [-4.7, 3.45, -5.72], size: [1.62, 0.98], sign: "vault" },
    { name: "CUP", at: [-1.6, 3.45, -5.72], size: [1.62, 0.98], sign: "cup" },
    { name: "FLOOR", at: [1.6, 3.45, -5.72], size: [1.62, 0.98], sign: "floor" },
    { name: "WAR", at: [4.7, 3.45, -5.72], size: [1.62, 0.98], sign: "war" },
  ],
  "cup-stadium": [{ name: "CUP", at: [0, 2.15, -1.2], size: [4.4, 1.15] }],
};
