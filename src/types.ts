export interface ImageInfo {
  path: string;
  width: number;
  height: number;
  /** Base64-encoded JPEG thumbnail */
  thumbnail: string;
}

export interface StackOptions {
  align: boolean;
  blend_radius: number;
}

export interface StackResult {
  width: number;
  height: number;
  /** Base64-encoded PNG preview */
  preview: string;
}

export type Status = "idle" | "loading" | "stacking" | "done" | "error";
