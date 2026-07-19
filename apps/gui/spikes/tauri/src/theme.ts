import type { ActionRecorder } from "./composer";

export enum Skin {
  AuroraDark = "aurora-dark",
  IceLight = "ice-light",
}

export enum Density {
  Compact = "compact",
  Regular = "regular",
  Comfy = "comfy",
}

export class ThemeModel {
  skin = Skin.AuroraDark;
  density = Density.Regular;

  constructor(private readonly record: ActionRecorder) {}

  select(skin: Skin, density: Density): void {
    this.skin = skin;
    this.density = density;
    this.record(`theme:${skin}:${density}`);
  }
}
