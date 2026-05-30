// Ambient declarations covering the Expo + react-native API surface this
// package consumes. Lets the contract test type-check without installing
// the full Expo SDK as a local dev dependency.

declare module "react-native" {
  export const Platform: { OS: "ios" | "android" | "web" | "windows" | "macos" };
  export const AppState: {
    addEventListener(event: "change", handler: (status: string) => void): { remove(): void };
  };
  export const BackHandler: {
    addEventListener(
      event: "hardwareBackPress",
      handler: () => boolean,
    ): { remove(): void };
  };
}

declare module "expo-linking" {
  export function addEventListener(
    event: "url",
    handler: (event: { url: string }) => void | Promise<void>,
  ): { remove(): void };
}

declare module "expo-notifications" {
  export function requestPermissionsAsync(): Promise<{ status: string }>;
  export function scheduleNotificationAsync(opts: {
    content: { title: string; body: string };
    trigger: null;
  }): Promise<string>;
  export function getExpoPushTokenAsync(): Promise<{ data: string }>;
  export function addNotificationReceivedListener(
    handler: (notif: { request: { content: { data: unknown } } }) => void,
  ): { remove(): void };
  export function addNotificationResponseReceivedListener(
    handler: (resp: { notification: { request: { content: { data: unknown } } } }) => void,
  ): { remove(): void };
}

declare module "expo-haptics" {
  export const ImpactFeedbackStyle: { Light: number; Medium: number; Heavy: number };
  export function impactAsync(style: number): Promise<void>;
}

declare module "expo-image-picker" {
  export const MediaTypeOptions: { Images: number; Videos: number; All: number };
  export function requestCameraPermissionsAsync(): Promise<{ status: string }>;
  export function launchCameraAsync(opts: {
    mediaTypes: number;
    quality: number;
  }): Promise<{ canceled: boolean; assets: { uri: string }[] }>;
}
