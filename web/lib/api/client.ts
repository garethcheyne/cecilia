import createClient from "openapi-fetch"

import type { components, paths } from "./schema"

export type Schemas = components["schemas"]
export type MidiStatus = Schemas["MidiStatus"]
export type MidiConfig = Schemas["MidiConfig"]
export type PortList = Schemas["PortList"]
export type EngineEvent = Schemas["EngineEvent"]
export type MidiEvent = Extract<EngineEvent, { type: "midi" }>
export type SimSnapshot = Schemas["SimSnapshot"]
export type OrganDefinition = Schemas["OrganDefinition"]
export type OrganSummary = Schemas["OrganSummary"]
export type OrganEvent = Schemas["OrganEvent"]
export type Stop = Schemas["Stop"]
export type Division = Schemas["Division"]
export type DivisionRole = Division["role"]
export type Track = Schemas["Track"]
export type TrackSummary = Schemas["TrackSummary"]
export type TrackInfo = Schemas["TrackInfo"]
export type TimedEvent = Schemas["TimedEvent"]
export type StopRef = Schemas["StopRef"]
export type StopMapping = Schemas["StopMapping"]
export type PlayerStatus = Schemas["PlayerStatus"]
export type PlayerState = Schemas["PlayerState"]
export type PlayOptions = Schemas["PlayOptions"]
export type RecorderStatus = Schemas["RecorderStatus"]

/** Empty string = same origin (production, served by organ-server). */
export const API_URL = process.env.NEXT_PUBLIC_API_URL ?? ""

export const api = createClient<paths>({ baseUrl: API_URL })

export function eventsUrl(): string {
  const base = API_URL || window.location.origin
  return `${base.replace(/^http/, "ws")}/api/v1/events`
}

export class ApiError extends Error {}

/** Unwrap an openapi-fetch result, turning API errors into exceptions. */
export async function call<T>(
  request: Promise<{ data?: T; error?: unknown; response: Response }>
): Promise<T> {
  let result
  try {
    result = await request
  } catch {
    throw new ApiError("Can't reach the Registra server. Is it running?")
  }
  if (result.error !== undefined) {
    const message =
      typeof result.error === "object" &&
      result.error !== null &&
      "error" in result.error
        ? String(result.error.error)
        : `Request failed (${result.response.status})`
    throw new ApiError(message)
  }
  return result.data as T
}
