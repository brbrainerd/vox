export type CatalogTier = 'recommended' | 'advanced' | 'feature_gated';

export interface CommandCatalogArgument {
  name: string;
  short: string | null;
  long: string | null;
  help: string | null;
  required: boolean;
  takes_value: boolean;
}

export interface CommandCatalogEntry {
  path: string[];
  command: string;
  about: string;
  aliases: string[];
  has_subcommands: boolean;
  compiled_in: boolean;
  source_group: string;
  feature_gate: string | null;
  tier: CatalogTier;
  capability_id?: string;
  arguments?: CommandCatalogArgument[];
}

export interface CommandCatalog {
  generated_from: string;
  entries: CommandCatalogEntry[];
}
