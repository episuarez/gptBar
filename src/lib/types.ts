// TypeScript types matching the Rust types

export interface RateWindow {
  used_percent: number;
  window_minutes: number | null;
  resets_at: string | null;
  reset_description: string | null;
}

export interface IdentitySnapshot {
  email: string | null;
  plan: string | null;
  organization: string | null;
}

export interface UsageSnapshot {
  primary: RateWindow | null;
  secondary: RateWindow | null;
  tertiary: RateWindow | null;
  updated_at: string;
  identity: IdentitySnapshot | null;
}

export type AuthMethod = 'OAuth' | 'Cookie' | 'Cli' | 'ApiToken' | 'None';

export interface ProviderMetadata {
  id: string;
  name: string;
  supports_login: boolean;
  auth_methods: AuthMethod[];
}

export interface ProviderState {
  id: string;
  name: string;
  snapshot: UsageSnapshot | null;
  loading: boolean;
  error: string | null;
  isAvailable: boolean;
}

export interface ProviderSettings {
  enabled: boolean;
  api_key?: string;
}

export interface AppConfig {
  refresh_interval: number;
  start_on_login: boolean;
  enabled_providers: string[];
  provider_settings: Record<string, ProviderSettings>;
}
