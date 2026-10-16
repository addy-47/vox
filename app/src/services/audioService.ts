import { invoke } from "./transport";

export interface AudioDevice {
  name: string;
  is_default: boolean;
}

export function listAudioDevices(kind: "input" | "output" = "input"): Promise<AudioDevice[]> {
  return invoke("list_audio_devices", { kind });
}

export function listInputDevices(): Promise<AudioDevice[]> {
  return listAudioDevices("input");
}

export function listOutputDevices(): Promise<AudioDevice[]> {
  return listAudioDevices("output");
}