export type ActionHandlerKind = 'cli' | 'mcp' | 'ipc';

export interface ActionPlatform {
  desktop: boolean;
  mobile: boolean;
}

export interface ActionArgument {
  name: string;
  short: string | null;
  long: string | null;
  help: string | null;
  required: boolean;
  takes_value: boolean;
}

export interface ActionManifestEntry {
  id: string;
  title: string;
  description: string;
  handler_kind: ActionHandlerKind;
  cli_path?: string[] | null;
  mcp_name?: string | null;
  command?: string | null;
  safety_class: string;
  feature_gate?: string | null;
  capability_id: string;
  scope_kind: string;
  requires_repo: boolean;
  reversible: boolean;
  confirmation_policy: string;
  execution_mode: string;
  output_kind: string;
  status: string;
  product_lane?: string | null;
  platform: ActionPlatform;
  arguments?: ActionArgument[];
}

export interface ActionManifest {
  x_vox_version: number;
  schema_version: number;
  generated_from: string;
  actions: ActionManifestEntry[];
}
