import { DISTRICTS } from "./city";

/** Synthesized beds and stings. Nothing is loaded from a sample pack. */

const KEY = "fe2vi.audio.v1";

export type Sting = "heist" | "breach" | "trade" | "rank" | "quest" | "forge";

type Prefs = { muted: boolean; volume: number; quiet: boolean };

const EMPTY: Prefs = { muted: false, volume: 0.7, quiet: false };

/** Warm A-minor pad. All partials stay under 250 Hz. Generated here, no sample file. */
const PAD = [110, 130.81, 164.81, 220];

function loadPrefs(): Prefs {
  if (typeof window === "undefined") return EMPTY;
  try {
    const raw = window.localStorage.getItem(KEY);
    if (!raw) {
      const quiet = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      return { ...EMPTY, quiet };
    }
    const parsed = JSON.parse(raw) as Partial<Prefs>;
    return {
      muted: Boolean(parsed.muted),
      volume: typeof parsed.volume === "number" ? Math.min(1, Math.max(0, parsed.volume)) : 0.7,
      quiet: Boolean(parsed.quiet),
    };
  } catch {
    return EMPTY;
  }
}

function savePrefs(prefs: Prefs) {
  window.localStorage.setItem(KEY, JSON.stringify(prefs));
}

class CityAudio {
  private ctx: AudioContext | null = null;
  private master: GainNode | null = null;
  private themeGain: GainNode | null = null;
  private themeVoices: OscillatorNode[] = [];
  private themeAir: AudioBufferSourceNode | null = null;
  private ambients: Array<{ id: string; gain: GainNode }> = [];
  private crowdGain: GainNode | null = null;
  private prefs: Prefs = EMPTY;
  private bed: "theme" | "city" = "theme";
  private heard = new Set<string>();
  private reduced = false;
  private blipAt = 0;

  hydrate(): Prefs {
    this.prefs = loadPrefs();
    return this.prefs;
  }

  running(): boolean {
    return Boolean(this.ctx);
  }

  /**
   * Button press. Creates the context so the click is audible, and does not
   * start the theme bed. Music stays on unlock(), which is Enter as spectator.
   */
  press() {
    if (typeof window === "undefined") return;
    this.prefs = loadPrefs();
    if (!this.ctx) {
      const Ctx = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctx) return;
      this.ctx = new Ctx();
      this.master = this.ctx.createGain();
      this.master.connect(this.ctx.destination);
      this.applyMaster();
    }
    void this.ctx.resume();
    this.click();
  }

  /** Call from a click. Skip does not call this. */
  unlock() {
    if (typeof window === "undefined") return;
    this.prefs = loadPrefs();
    this.reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (!this.ctx) {
      const Ctx = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctx) return;
      this.ctx = new Ctx();
      this.master = this.ctx.createGain();
      this.master.connect(this.ctx.destination);
      this.buildBeds();
    }
    void this.ctx.resume();
    this.applyMaster();
    this.intro();
    if (this.bed === "theme") this.startTheme();
    else this.raiseCity();
  }

  setMuted(muted: boolean) {
    this.prefs = { ...this.prefs, muted };
    if (typeof window !== "undefined") savePrefs(this.prefs);
    this.applyMaster();
  }

  setVolume(volume: number) {
    this.prefs = { ...this.prefs, volume: Math.min(1, Math.max(0, volume)) };
    if (typeof window !== "undefined") savePrefs(this.prefs);
    this.applyMaster();
  }

  setQuiet(quiet: boolean) {
    this.prefs = { ...this.prefs, quiet };
    if (typeof window !== "undefined") savePrefs(this.prefs);
    this.applyMaster();
  }

  swell(beat: number) {
    if (!this.themeGain || !this.ctx) return;
    const level = Math.min(0.85, 0.28 + beat * 0.12);
    this.themeGain.gain.setTargetAtTime(this.prefs.quiet ? level * 0.35 : level, this.ctx.currentTime, 0.25);
  }

  theme() {
    this.bed = "theme";
    if (!this.ctx) return;
    this.startTheme();
    this.ambients.forEach((bed) => {
      bed.gain.gain.value = 0.02;
    });
  }

  city() {
    this.bed = "city";
    if (!this.ctx) return;
    this.stopTheme();
    this.raiseCity();
  }

  listener(x: number, y: number, z: number, fx: number, fy: number, fz: number) {
    const ctx = this.ctx;
    if (!ctx) return;
    const listener = ctx.listener;
    listener.positionX.value = x;
    listener.positionY.value = y;
    listener.positionZ.value = z;
    listener.forwardX.value = fx;
    listener.forwardY.value = fy;
    listener.forwardZ.value = fz;
    listener.upX.value = 0;
    listener.upY.value = 1;
    listener.upZ.value = 0;
  }

  blip(x: number, y: number, z: number, now: number) {
    if (!this.ctx || !this.master || this.prefs.muted || this.prefs.quiet || now < this.blipAt) return;
    this.blipAt = now + 0.55;
    this.tone(this.ctx.currentTime, 520, 0.05, 0.04, "square", x, y, z);
  }

  hover() {
    if (!this.ctx || this.prefs.muted) return;
    this.tone(this.ctx.currentTime, 880, 0.05, 0.12, "triangle");
  }

  click() {
    if (!this.ctx || !this.master || this.prefs.muted) return;
    const ctx = this.ctx;
    const t = ctx.currentTime;
    const osc = ctx.createOscillator();
    const filter = ctx.createBiquadFilter();
    const gain = ctx.createGain();
    osc.type = "sine";
    osc.frequency.setValueAtTime(280, t);
    osc.frequency.exponentialRampToValueAtTime(160, t + 0.04);
    filter.type = "lowpass";
    filter.frequency.value = 700;
    gain.gain.setValueAtTime(0.0001, t);
    gain.gain.exponentialRampToValueAtTime(0.045, t + 0.006);
    gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.045);
    osc.connect(filter).connect(gain).connect(this.master);
    osc.start(t);
    osc.stop(t + 0.05);
  }

  intro() {
    if (!this.ctx || !this.master || this.prefs.muted || this.heard.has("intro")) return;
    this.heard.add("intro");
    const t = this.ctx.currentTime;
    [196, 247, 294, 392, 494, 392].forEach((freq, index) => {
      this.tone(t + index * 0.16, freq, 0.42, 0.22, "triangle");
    });
  }

  whoosh() {
    if (!this.ctx || !this.master || this.prefs.muted) return;
    const ctx = this.ctx;
    const noise = ctx.createBuffer(1, ctx.sampleRate * 0.4, ctx.sampleRate);
    const data = noise.getChannelData(0);
    for (let i = 0; i < data.length; i += 1) data[i] = Math.random() * 2 - 1;
    const source = ctx.createBufferSource();
    source.buffer = noise;
    const filter = ctx.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.setValueAtTime(400, ctx.currentTime);
    filter.frequency.exponentialRampToValueAtTime(1800, ctx.currentTime + 0.35);
    const gain = ctx.createGain();
    gain.gain.setValueAtTime(0.0001, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.35, ctx.currentTime + 0.08);
    gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + 0.4);
    source.connect(filter).connect(gain).connect(this.master);
    source.start();
    source.stop(ctx.currentTime + 0.42);
  }

  alarm() {
    if (!this.ctx || !this.master || this.prefs.muted) return;
    const t = this.ctx.currentTime;
    [0, 0.28, 0.56].forEach((offset) => this.tone(t + offset, 740, 0.16, 0.2, "sawtooth"));
  }

  crowd(on: boolean) {
    if (!this.ctx || !this.master) return;
    if (!on) {
      this.crowdGain?.disconnect();
      this.crowdGain = null;
      return;
    }
    if (this.crowdGain || this.prefs.muted) return;
    const ctx = this.ctx;
    const noise = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate);
    const data = noise.getChannelData(0);
    for (let i = 0; i < data.length; i += 1) data[i] = Math.random() * 2 - 1;
    const source = ctx.createBufferSource();
    source.buffer = noise;
    source.loop = true;
    const filter = ctx.createBiquadFilter();
    filter.type = "bandpass";
    filter.frequency.value = 500;
    filter.Q.value = 0.6;
    const gain = ctx.createGain();
    gain.gain.value = this.prefs.quiet ? 0.08 : 0.22;
    source.connect(filter).connect(gain).connect(this.master);
    source.start();
    this.crowdGain = gain;
  }

  /**
   * Short "iron arena" motif. Opens the context the way a button press
   * does, and does not start the theme bed.
   */
  pronounce() {
    if (typeof window === "undefined") return;
    this.prefs = loadPrefs();
    if (!this.ctx) {
      const Ctx = window.AudioContext || (window as Window & { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
      if (!Ctx) return;
      this.ctx = new Ctx();
      this.master = this.ctx.createGain();
      this.master.connect(this.ctx.destination);
      this.applyMaster();
    }
    void this.ctx.resume();
    if (this.prefs.muted || !this.master) return;
    const t = this.ctx.currentTime;
    const duck = this.prefs.quiet ? 0.35 : 1;
    this.tone(t, 330, 0.16, 0.07 * duck, "triangle");
    this.tone(t + 0.14, 247, 0.2, 0.06 * duck, "triangle");
    this.tone(t + 0.42, 392, 0.1, 0.05 * duck, "sine");
    this.tone(t + 0.52, 440, 0.1, 0.05 * duck, "sine");
    this.tone(t + 0.64, 523, 0.18, 0.06 * duck, "sine");
  }

  sting(name: Sting) {
    if (!this.ctx || !this.master || this.prefs.muted || this.heard.has(name)) return;
    this.heard.add(name);
    const duck = this.prefs.quiet || this.reduced ? 0.25 : 1;
    const t = this.ctx.currentTime;
    const notes: Record<Sting, number[]> = {
      heist: [392, 494, 587],
      breach: [196, 185, 174],
      trade: [523, 659],
      rank: [440, 554, 659],
      quest: [349, 440],
      forge: [165, 220, 330],
    };
    notes[name].forEach((freq, index) => {
      this.tone(t + index * 0.12, freq, 0.18, 0.2 * duck, name === "breach" ? "sawtooth" : "triangle");
    });
  }

  private applyMaster() {
    if (!this.master || !this.ctx) return;
    const level = this.prefs.muted ? 0 : this.prefs.volume * (this.prefs.quiet ? 0.35 : 1);
    this.master.gain.setTargetAtTime(level, this.ctx.currentTime, 0.03);
  }

  private buildBeds() {
    const ctx = this.ctx;
    const master = this.master;
    if (!ctx || !master) return;
    const noise = ctx.createBuffer(1, ctx.sampleRate * 2, ctx.sampleRate);
    const data = noise.getChannelData(0);
    for (let i = 0; i < data.length; i += 1) data[i] = Math.random() * 2 - 1;

    DISTRICTS.forEach((district) => {
      const source = ctx.createBufferSource();
      source.buffer = noise;
      source.loop = true;
      const filter = ctx.createBiquadFilter();
      filter.type = district.id === "war-room" ? "bandpass" : "lowpass";
      filter.frequency.value =
        district.id === "cup-stadium" ? 900 : district.id === "trading-village" ? 1400 : district.id === "war-room" ? 220 : 180;
      filter.Q.value = district.id === "war-room" ? 4 : 0.7;
      const tone = ctx.createOscillator();
      tone.type = "sine";
      tone.frequency.value =
        district.id === "vault-quarter" ? 55 : district.id === "war-room" ? 146 : district.id === "skin-forge" ? 70 : 82;
      const gain = ctx.createGain();
      gain.gain.value = 0;
      const panner = ctx.createPanner();
      panner.panningModel = "HRTF";
      panner.distanceModel = "inverse";
      panner.refDistance = 6;
      panner.rolloffFactor = 1.6;
      panner.maxDistance = 48;
      panner.positionX.value = district.x;
      panner.positionY.value = 1.2;
      panner.positionZ.value = district.z;
      source.connect(filter).connect(gain);
      tone.connect(gain);
      gain.connect(panner).connect(master);
      source.start();
      tone.start();
      this.ambients.push({ id: district.id, gain });
    });
  }

  private startTheme() {
    const ctx = this.ctx;
    const master = this.master;
    if (!ctx || !master || this.themeGain) return;
    const gain = ctx.createGain();
    gain.gain.value = this.prefs.quiet ? 0.03 : 0.06;
    gain.connect(master);
    this.themeGain = gain;
    const filter = ctx.createBiquadFilter();
    filter.type = "lowpass";
    filter.frequency.value = 640;
    filter.Q.value = 0.35;
    filter.connect(gain);
    PAD.forEach((freq, index) => {
      const osc = ctx.createOscillator();
      osc.type = index % 2 === 0 ? "sine" : "triangle";
      osc.frequency.value = freq;
      osc.detune.value = (index - 1.5) * 5;
      const voice = ctx.createGain();
      voice.gain.value = 0.16;
      osc.connect(voice).connect(filter);
      osc.start();
      this.themeVoices.push(osc);
    });
    const breath = ctx.createOscillator();
    breath.frequency.value = 0.07;
    const breathDepth = ctx.createGain();
    breathDepth.gain.value = 0.012;
    breath.connect(breathDepth).connect(gain.gain);
    breath.start();
    this.themeVoices.push(breath);
    const seconds = 4;
    const noise = ctx.createBuffer(1, Math.floor(ctx.sampleRate * seconds), ctx.sampleRate);
    const data = noise.getChannelData(0);
    let last = 0;
    for (let i = 0; i < data.length; i += 1) {
      last = (last + 0.02 * (Math.random() * 2 - 1)) / 1.02;
      data[i] = last * 3.2;
    }
    const fade = Math.floor(ctx.sampleRate * 0.03);
    for (let i = 0; i < fade; i += 1) {
      const edge = i / fade;
      data[i] *= edge;
      data[data.length - 1 - i] *= edge;
    }
    const air = ctx.createBufferSource();
    air.buffer = noise;
    air.loop = true;
    const airFilter = ctx.createBiquadFilter();
    airFilter.type = "lowpass";
    airFilter.frequency.value = 380;
    const airGain = ctx.createGain();
    airGain.gain.value = 0.035;
    air.connect(airFilter).connect(airGain).connect(filter);
    air.start();
    this.themeAir = air;
  }

  private stopTheme() {
    this.themeVoices.forEach((osc) => {
      try {
        osc.stop();
      } catch {
        /* already stopped */
      }
      osc.disconnect();
    });
    this.themeVoices = [];
    if (this.themeAir) {
      try {
        this.themeAir.stop();
      } catch {
        /* already stopped */
      }
      this.themeAir.disconnect();
      this.themeAir = null;
    }
    this.themeGain?.disconnect();
    this.themeGain = null;
  }

  private raiseCity() {
    this.ambients.forEach((bed) => {
      const level = 0.28;
      bed.gain.gain.value = this.prefs.quiet ? level * 0.4 : level;
    });
  }

  private tone(
    when: number,
    freq: number,
    duration: number,
    level: number,
    type: OscillatorType,
    x?: number,
    y?: number,
    z?: number,
    dest?: AudioNode,
  ) {
    const ctx = this.ctx;
    const master = this.master;
    if (!ctx || !master) return;
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = type;
    osc.frequency.value = freq;
    gain.gain.setValueAtTime(level, when);
    gain.gain.exponentialRampToValueAtTime(0.0001, when + duration);
    osc.connect(gain);
    const output = dest ?? master;
    if (x !== undefined && y !== undefined && z !== undefined) {
      const panner = ctx.createPanner();
      panner.panningModel = "HRTF";
      panner.distanceModel = "inverse";
      panner.refDistance = 2;
      panner.positionX.value = x;
      panner.positionY.value = y;
      panner.positionZ.value = z;
      gain.connect(panner).connect(output);
    } else {
      gain.connect(output);
    }
    osc.start(when);
    osc.stop(when + duration + 0.02);
  }
}

export const audio = new CityAudio();
