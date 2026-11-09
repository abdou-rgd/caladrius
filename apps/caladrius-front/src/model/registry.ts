// What the page reads from the engine's registry (`describe()`): the commands, and the enums of `fit.run`.

export interface CommandInfo {
  id: string;
  /** The first sentence of the description (the registry has no separate title). */
  title: string;
  description: string;
  mutates: boolean;
  /** Names of the required parameters. */
  required: string[];
  example: unknown;
}

interface RawCommand {
  id?: unknown;
  description?: unknown;
  mutates?: unknown;
  params_schema?: { required?: unknown; $defs?: Record<string, { enum?: unknown }> };
  example?: unknown;
}

function firstSentence(text: string): string {
  const match = /^(.+?[.!?])(\s|$)/s.exec(text);
  const sentence = (match?.[1] ?? text).replace(/\s+/g, ' ').trim();
  return sentence.length > 110 ? `${sentence.slice(0, 107)}...` : sentence;
}

export function commandInfos(describe: unknown): CommandInfo[] {
  if (!Array.isArray(describe)) return [];
  const out: CommandInfo[] = [];
  for (const raw of describe as RawCommand[]) {
    if (typeof raw?.id !== 'string') continue;
    const description = typeof raw.description === 'string' ? raw.description : '';
    const required = Array.isArray(raw.params_schema?.required)
      ? (raw.params_schema.required as unknown[]).filter((x): x is string => typeof x === 'string')
      : [];
    out.push({ id: raw.id, title: firstSentence(description), description, mutates: raw.mutates === true, required, example: raw.example ?? null });
  }
  return out;
}

/** An enum of a command's schema: `enumOf(describe, 'fit.run', 'ModelId')`. Empty when the registry does not have it. */
export function enumOf(describe: unknown, command: string, def: string): string[] {
  if (!Array.isArray(describe)) return [];
  const found = (describe as RawCommand[]).find((c) => c?.id === command);
  const values = found?.params_schema?.$defs?.[def]?.enum;
  return Array.isArray(values) ? values.filter((x): x is string => typeof x === 'string') : [];
}
