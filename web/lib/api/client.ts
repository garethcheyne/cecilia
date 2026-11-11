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
export type Playlist = Schemas["Playlist"]
export type PlaylistRow = Schemas["PlaylistRow"]
export type PlaylistSummary = Schemas["PlaylistSummary"]
export type Delay = Schemas["Delay"]
export type VersePlan = Schemas["VersePlan"]
export type ServiceStatus = Schemas["ServiceStatus"]
export type ImportAnalysis = Schemas["ImportAnalysis"]
export type LiveSnapshot = Schemas["LiveSnapshot"]
export type Manufacturer = OrganDefinition["manufacturer"]
export type SettingsBody = Schemas["SettingsBody"]
export type Catalogue = Schemas["Catalogue"]
export type MusicEntry = Schemas["MusicEntry"]
export type OrganEntry = Schemas["OrganEntry"]
export type ImportOptions = Schemas["ImportOptions"]
export type XmlImportOptions = Schemas["XmlImportOptions"]

/** Absolute URL of an API path, for downloads and uploads. */
export function apiUrl(path: string): string {
  return `${API_URL}${path}`
}

/** POST a file's bytes to an import endpoint. */
export async function uploadFile<T>(
  path: string,
  file: File,
  options?: unknown
): Promise<T> {
  const params = new URLSearchParams({ name: file.name })
  if (options !== undefined) params.set("options", JSON.stringify(options))
  let res: Response
  try {
    res = await fetch(apiUrl(`${path}?${params}`), {
      method: "POST",
      body: file,
      credentials: "include",
    })
  } catch {
    throw new ApiError("Can't reach the Cecilia server. Is it running?")
  }
  const body = await res.json().catch(() => null)
  if (!res.ok)
    throw new ApiError(body?.error ?? `Request failed (${res.status})`)
  return body as T
}

/** Empty string = same origin (production, served by organ-server). */
export const API_URL = process.env.NEXT_PUBLIC_API_URL ?? ""

export const api = createClient<paths>({
  baseUrl: API_URL,
  credentials: "include",
})

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
    throw new ApiError("Can't reach the Cecilia server. Is it running?")
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
export type Organist = Schemas["Organist"]
export type OrganistsBody = Schemas["OrganistsBody"]
export type CommunityAccountStatus = Schemas["CommunityAccountStatus"]
export type ShareResult = Schemas["ShareResult"]
export type UpdateStatus = Schemas["UpdateStatus"]
export type UpdateSettings = Schemas["UpdateSettings"]
