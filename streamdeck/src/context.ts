// What every action shares: the connection settings, one client and one monitor.

import streamDeck from "@elgato/streamdeck";

import { Monitor } from "./monitor.ts";
import { DEFAULT_CONNECTION, RoomtoneClient, type Connection } from "./roomtone.ts";

let connection: Connection = { ...DEFAULT_CONNECTION };

export const client = new RoomtoneClient(() => connection);
export const monitor = new Monitor(client);

/** Read the connection (host, port, access key) the person typed into any button's settings. */
export function applyGlobalSettings(settings: Partial<Connection> | undefined): void {
  const port = Number(settings?.port);
  connection = {
    host: (settings?.host ?? "").toString().trim() || DEFAULT_CONNECTION.host,
    port: Number.isInteger(port) && port > 0 && port < 65536 ? port : DEFAULT_CONNECTION.port,
    key: (settings?.key ?? "").toString().trim(),
  };
  void monitor.refresh();
}

export async function loadGlobalSettings(): Promise<void> {
  applyGlobalSettings(await streamDeck.settings.getGlobalSettings<Partial<Connection>>());
  streamDeck.settings.onDidReceiveGlobalSettings<Partial<Connection>>((ev) => applyGlobalSettings(ev.settings));
}
