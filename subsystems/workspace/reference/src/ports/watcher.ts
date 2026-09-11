/**
 * WatcherPort — filesystem change events feeding re-indexing (SDD §6.4).
 *
 * Debounce policy lives behind the port (in the adapter), not in the tool
 * layer; subscribers see coalesced events.
 */

import type { PORT_VERSIONS } from "./versions";

export type WatchEventKind = "created" | "modified" | "removed";

export interface WatchEvent {
  readonly kind: WatchEventKind;
  readonly path: string;
}

export interface WatchSubscription {
  close(): Promise<void>;
}

export interface WatcherPort {
  readonly version: typeof PORT_VERSIONS.watcher;
  subscribe(
    paths: readonly string[],
    onEvent: (event: WatchEvent) => void,
  ): Promise<WatchSubscription>;
}
