import type { ArgsSchema, ToolArgs } from './types';

// eslint-disable-next-line no-control-regex
const CONTROL_CHARS = /[\u0000-\u001f\u007f]/;

export interface ValidationResult {
  ok: boolean;
  value: ToolArgs;
  errors: string[];
}

/**
 * Validates untrusted args against a tiny schema. Unknown keys are dropped,
 * strings are trimmed, control characters are rejected.
 */
export function validateArgs(schema: ArgsSchema, raw: unknown): ValidationResult {
  const errors: string[] = [];
  const value: ToolArgs = {};
  const src: Record<string, unknown> =
    raw && typeof raw === 'object' && !Array.isArray(raw) ? (raw as Record<string, unknown>) : {};

  for (const [key, spec] of Object.entries(schema)) {
    const v = src[key];
    if (v === undefined || v === null || v === '') {
      if (spec.required) errors.push(`${key} is required`);
      continue;
    }
    if (typeof v !== 'string') {
      errors.push(`${key} must be a string`);
      continue;
    }
    const s = v.trim();
    if (s === '') {
      if (spec.required) errors.push(`${key} is required`);
      continue;
    }
    if (CONTROL_CHARS.test(s)) {
      errors.push(`${key} contains control characters`);
      continue;
    }
    if (spec.maxLen !== undefined && s.length > spec.maxLen) {
      errors.push(`${key} is longer than ${spec.maxLen} characters`);
      continue;
    }
    if (spec.type === 'enum' && !(spec.values || []).includes(s)) {
      errors.push(`${key} must be one of ${(spec.values || []).join(', ')}`);
      continue;
    }
    if (spec.pattern && !spec.pattern.test(s)) {
      errors.push(`${key} has an invalid format`);
      continue;
    }
    value[key] = s;
  }

  return { ok: errors.length === 0, value, errors };
}
