import { scValToNative, xdr, Address } from "@stellar/stellar-sdk";

/**
 * Turning contract values into something a database column can hold.
 *
 * Two rules run through all of it. Amounts and voting power are i128 and u128,
 * so they become strings rather than numbers; rounding a treasury balance to
 * fit a double is not a trade this product can make. And bytes become hex,
 * because a proposal identifier is something a member copies and compares.
 */

export type Json = string | number | boolean | null | Json[] | { [k: string]: Json };

/** Converts a contract value into JSON-safe data. */
export function decode(value: xdr.ScVal): Json {
  return normalise(scValToNative(value));
}

function normalise(value: unknown): Json {
  if (value === null || value === undefined) return null;

  switch (typeof value) {
    case "bigint":
      return value.toString();
    case "string":
    case "number":
    case "boolean":
      return value;
  }

  if (value instanceof Uint8Array) return hex(value);
  if (Array.isArray(value)) return value.map(normalise);

  if (value instanceof Map) {
    const out: { [k: string]: Json } = {};
    for (const [key, item] of value.entries()) out[String(key)] = normalise(item);
    return out;
  }

  if (typeof value === "object") {
    // scValToNative gives addresses back as strings already; anything else
    // that reaches here is a struct, which arrives as a plain object.
    const out: { [k: string]: Json } = {};
    for (const [key, item] of Object.entries(value as object)) out[key] = normalise(item);
    return out;
  }

  return String(value);
}

export function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

/** The first topic of an event, which is the event's name. */
export function topicName(topics: xdr.ScVal[]): string {
  const first = topics[0];
  if (!first) return "";
  try {
    const native = scValToNative(first);
    return typeof native === "string" ? native : String(native);
  } catch {
    return "";
  }
}

/** A topic read as an address, for the events that key on one. */
export function topicAddress(topics: xdr.ScVal[], index: number): string {
  const topic = topics[index];
  if (!topic) return "";
  try {
    return Address.fromScVal(topic).toString();
  } catch {
    const decoded = decode(topic);
    return typeof decoded === "string" ? decoded : "";
  }
}

/** A topic read as a 32 byte identifier, in hex. */
export function topicHash(topics: xdr.ScVal[], index: number): string {
  const topic = topics[index];
  if (!topic) return "";
  const decoded = decode(topic);
  return typeof decoded === "string" ? decoded : "";
}

/** The body of an event, as a named record where the event has one. */
export function decodeBody(value: xdr.ScVal): { [k: string]: Json } {
  const decoded = decode(value);
  if (decoded !== null && typeof decoded === "object" && !Array.isArray(decoded)) {
    return decoded;
  }
  return { value: decoded };
}

export function asString(value: Json | undefined): string {
  if (value === null || value === undefined) return "";
  return typeof value === "string" ? value : String(value);
}

export function asNumber(value: Json | undefined): number {
  if (value === null || value === undefined) return 0;
  const n = Number(value);
  return Number.isFinite(n) ? n : 0;
}
