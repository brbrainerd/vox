# Crate API: vox-config

## Module: `vox-config\src\lib.rs`

Centralized configuration for Vox: env vars, defaults, and path resolution.

Precedence: CLI args > env > config file > defaults.


## Module: `vox-config\src\paths.rs`

Cross-platform path and directory resolution.

Single source of truth for VOX_DATA_DIR, VOX_USER_ID, and platform data dirs.
Precedence: env vars > platform defaults.


### `fn data_dir`

Resolve the Vox data directory. Env `VOX_DATA_DIR` overrides; else platform default.


### `fn default_db_path`

Default database path: `<data_dir>/vox.db`.


### `fn state_dir`

State directory for durable objects: `<data_dir>/state/`.


### `fn config_dir`

Config directory: `<data_dir>/config/`.


### `fn local_user_id`

Current user id for local usage. Env `VOX_USER_ID` or platform username or `"local-user"`.


