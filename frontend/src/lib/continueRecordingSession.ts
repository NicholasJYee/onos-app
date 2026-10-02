/**
 * Tracks whether a "continue recording" session is in progress.
 *
 * A resumed recording is owned by the meeting page: it appends to the meeting
 * it continues rather than creating a new one. The global post-processing
 * provider in `app/layout.tsx` reacts to `recording-stop-complete` (emitted
 * when a recording is stopped from the tray menu) by running the normal save
 * flow, which creates a *new* meeting. Without this flag, stopping a resumed
 * recording from the tray would leave behind a duplicate.
 *
 * Deliberately module state rather than React context: the flag is read inside
 * a Tauri event listener that is set up once, outside the meeting page's tree.
 */
let active = false;

export function setContinueSessionActive(value: boolean): void {
  active = value;
}

export function isContinueSessionActive(): boolean {
  return active;
}
