/**
 * Default local transcription model, by platform.
 *
 * iOS uses `small` (~466 MB) rather than large-v3-turbo (~1.5 GB): the larger
 * model is a heavy first-run download on a phone and competes for memory with
 * the Gemma summary model. Mirrors DEFAULT_WHISPER_MODEL in
 * src-tauri/src/audio/transcription/engine.rs.
 */

export const DESKTOP_TRANSCRIPTION_MODEL = 'large-v3-turbo';
export const IOS_TRANSCRIPTION_MODEL = 'small';

/** Every model this app may download, for matching progress events. */
export const TRANSCRIPTION_MODELS = [
  DESKTOP_TRANSCRIPTION_MODEL,
  IOS_TRANSCRIPTION_MODEL,
];

/** Approximate on-disk size in MB, used for progress display. */
export const TRANSCRIPTION_MODEL_SIZE_MB: Record<string, number> = {
  [DESKTOP_TRANSCRIPTION_MODEL]: 1549,
  [IOS_TRANSCRIPTION_MODEL]: 466,
};

/** True when the event refers to whichever transcription model we download. */
export function isTranscriptionModel(name: string | undefined): boolean {
  return !!name && TRANSCRIPTION_MODELS.includes(name);
}

/** Resolve the default model for the current platform. */
export async function getDefaultTranscriptionModel(): Promise<string> {
  try {
    const { platform } = await import('@tauri-apps/plugin-os');
    return platform() === 'ios'
      ? IOS_TRANSCRIPTION_MODEL
      : DESKTOP_TRANSCRIPTION_MODEL;
  } catch {
    return DESKTOP_TRANSCRIPTION_MODEL;
  }
}
