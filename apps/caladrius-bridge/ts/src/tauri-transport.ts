// The Tauri transport: the engine runs in the Rust process of the shell; the page calls the one
// command `engine_execute` (the only one its capability allows).

import { TextClient, TransportError } from './protocol.js';

/** The name of the command of the shell (`apps/caladrius-tauri`). */
export const ENGINE_COMMAND = 'engine_execute';

/** `invoke` of `@tauri-apps/api/core`, or a stand-in. */
export type Invoke = (command: string, args: Record<string, unknown>) => Promise<unknown>;

async function tauriInvoke(command: string, args: Record<string, unknown>): Promise<unknown> {
  // Loaded when the first request goes out, so a page that uses the worker transport does not
  // load the Tauri API.
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke(command, args);
}

export class TauriTransport extends TextClient {
  constructor(private readonly invoke: Invoke = tauriInvoke) {
    super();
  }

  async send(request: string): Promise<string> {
    let answer: unknown;
    try {
      answer = await this.invoke(ENGINE_COMMAND, { request });
    } catch (cause) {
      throw new TransportError(`the Tauri command ${ENGINE_COMMAND} failed: ${String(cause)}`, { cause });
    }
    if (typeof answer !== 'string') {
      throw new TransportError(`the Tauri command ${ENGINE_COMMAND} did not answer with text`);
    }
    return answer;
  }

  close(): void {}
}
