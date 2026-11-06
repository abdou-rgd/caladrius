// The one call and its answer. The shape is the one documented in the doc comment of the Rust
// crate `caladrius-bridge`: every answer has all six keys, a command that fails has the same shape
// as one that works.

/** The seal of a command: the hash of what it did, plus provenance when the engine adds it. */
export interface Seal {
  format: number;
  sha256: string;
  engine?: string;
  commit?: string;
  input?: string;
}

/** The error of a command that failed (`code` is stable, `message` is for a person). */
export interface EngineError {
  code: string;
  message: string;
  [key: string]: unknown;
}

/** The answer to one request. `ok` tells which of `result` and `error` is set. */
export interface Answer<Result = unknown> {
  /** The time of the command, when the host gave the engine a clock; null otherwise. */
  at: string | null;
  /** The command that ran, or null when the request was not readable. */
  command: string | null;
  error: EngineError | null;
  ok: boolean;
  result: Result | null;
  /** The seal of the history entry the command recorded; null when it recorded none. */
  seal: Seal | null;
}

/** The engine as the page sees it, whatever carries the request. */
export interface EngineClient {
  /** Runs one command and gives its answer. A command that fails still gives an answer. */
  execute<Result = unknown>(command: string, params?: unknown): Promise<Answer<Result>>;
  /** The call as the bridge has it: request text in, answer text out. */
  send(request: string): Promise<string>;
  /** Lets go of the transport (terminates the worker). */
  close(): void;
}

/** The transport failed: the engine was not reached or its answer is not an answer. */
export class TransportError extends Error {
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = 'TransportError';
  }
}

const ANSWER_KEYS = ['at', 'command', 'error', 'ok', 'result', 'seal'] as const;

/** The request text of a command. */
export function encodeRequest(command: string, params?: unknown): string {
  return JSON.stringify(params === undefined ? { command } : { command, params });
}

/** Reads the answer text of the bridge; refuses text that does not have the fixed shape. */
export function decodeAnswer<Result = unknown>(text: string): Answer<Result> {
  let value: unknown;
  try {
    value = JSON.parse(text);
  } catch (cause) {
    throw new TransportError('the answer is not JSON', { cause });
  }
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new TransportError('the answer is not an object');
  }
  const record = value as Record<string, unknown>;
  for (const key of ANSWER_KEYS) {
    if (!(key in record)) {
      throw new TransportError(`the answer has no "${key}"`);
    }
  }
  if (typeof record['ok'] !== 'boolean') {
    throw new TransportError('the answer has no boolean "ok"');
  }
  return value as Answer<Result>;
}

/** What a transport has to give: the text call. The rest is the same for all. */
export abstract class TextClient implements EngineClient {
  abstract send(request: string): Promise<string>;
  abstract close(): void;

  async execute<Result = unknown>(command: string, params?: unknown): Promise<Answer<Result>> {
    return decodeAnswer<Result>(await this.send(encodeRequest(command, params)));
  }
}
