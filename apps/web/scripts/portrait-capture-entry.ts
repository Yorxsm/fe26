import { capturePortraitPng } from "../lib/portraitCapture";
import { defaultSkin } from "../lib/skin";

declare global {
  interface Window {
    capturePortrait: (tokenId: string) => Promise<string>;
  }
}

window.capturePortrait = (tokenId: string) => capturePortraitPng(defaultSkin(tokenId));
