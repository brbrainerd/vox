---
title: "Standard Library Built-ins"
description: "Core execution environment capabilities exposed in Vox (std.* and built-ins)."
category: "Language Reference"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# Reference: Standard Library Built-ins

Vox includes a minimal, highly optimized standard library focused exclusively on system I/O, core conversions, and process lifecycle capabilities inherently trusted by the compiler orchestrator.

## Global Built-ins

These core functions are evaluated globally across any lexical space in the application without module imports.

| Signature | Description |
|-----------|-------------|
| `fn len(collection: T) to int` | Returns the number of elements in a sequence, string, list, or mapping dictionary structure. |
| `fn str(val: T) to str` | Explicitly coerces arbitrary object types and scalar values strictly into UTF-8 strings. |
| `fn assert(condition: bool) to Unit` | Halts execution contexts raising terminal logic failures safely. |
| `fn print(message: str) to Unit` | Synchronous STDOUT writer. |

## Process and Execution IO (`std.fs.*`)

File system operations interact securely via WASI/os permission mappings. Error cascades explicitly require `Result`.

| Signature | Description |
|-----------|-------------|
| `fn read(path: str) to Result[str]` | Reads file at `path` as UTF-8 text. Returns `Error(msg)` if not found or unreadable. |
| `fn write(path: str, content: str) to Result[Unit]` | Creates or completely overwrites the target file with the string content. |
| `fn exists(path: str) to bool` | Evaluates whether a file or directory exists at the given path. |
| `fn is_file(path: str) to bool` | Returns true if the path is a file. |
| `fn is_dir(path: str) to bool` | Returns true if the path is a directory. |
| `fn canonicalize(path: str) to Result[str]` | Returns the canonical, absolute form of the path. |
| `fn list_dir(path: str) to Result[list[str]]` | Returns a list of filenames in the directory. |
| `fn glob(pattern: str) to Result[list[str]]` | Returns a list of paths matching the glob pattern. |
| `fn remove(path: str) to Result[Unit]` | Removes the file at the given path. |
| `fn read_bytes(path: str) to Result[str]` | Reads raw bytes as a string representation. |
| `fn mkdir(path: str) to Result[Unit]` | Creates a single directory at the given path. |
| `fn copy(src: str, dst: str) to Result[Unit]` | Copies a file from source to destination. |
| `fn remove_dir_all(path: str) to Result[Unit]` | Recursively removes a directory and all of its contents. |

## Path Manipulation (`std.path.*`)

| Signature | Description |
|-----------|-------------|
| `fn join(a: str, b: str) to str` | Joins two path parts. |
| `fn join_many(parts: list[str]) to str` | Joins a list of path parts. |
| `fn basename(p: str) to str` | Extracts the base name from a path. |
| `fn dirname(p: str) to str` | Extracts the directory name from a path. |
| `fn extension(p: str) to str` | Extracts the file extension. |

## Environment (`std.env.*`)

| Signature | Description |
|-----------|-------------|
| `fn get(key: str) to Option[str]` | Retrieves an environment variable. |

## Process Execution (`std.process.*`)

| Signature | Description |
|-----------|-------------|
| `fn which(cmd: str) to Option[str]` | Finds a command in the PATH. |
| `fn run(cmd: str, args: list[str]) to Result[int]` | Runs a command and returns the exit code. |
| `fn run_ex(cmd: str, args: list[str], cwd: str, env: map[str, str]) to Result[int]` | Runs a command with specific cwd and environment. |
| `fn run_capture(cmd: str, args: list[str]) to Result[{exit: int, stdout: str, stderr: str}]` | Runs a command and captures its output. |
| `fn exit(code: int) to never` | Terminates the process with the given exit code. |

## JSON Processing (`std.json.*`)

| Signature | Description |
|-----------|-------------|
| `fn read_str(json: str, path: str) to Result[str]` | Extracts a string from a JSON document at the given path. |
| `fn read_f64(json: str, path: str) to Result[float]` | Extracts a float from JSON. |
| `fn quote(s: str) to str` | Properly escapes a string for inclusion in JSON. |

## Cryptography (`std.crypto.*`)

| Signature | Description |
|-----------|-------------|
| `fn hash_fast(s: str) to str` | Fast, non-cryptographic hash. |
| `fn hash_secure(s: str) to str` | Secure cryptographic hash (SHA-256). |
| `fn uuid() to str` | Generates a UUID v4 string. |

## Time (`std.time.*`)

| Signature | Description |
|-----------|-------------|
| `fn now_ms() to int` | Returns current UNIX timestamp in milliseconds. |

## Logging (`std.log.*`)

| Signature | Description |
|-----------|-------------|
| `fn debug(msg: str) to Unit` | Logs a debug message. |
| `fn info(msg: str) to Unit` | Logs an info message. |
| `fn warn(msg: str) to Unit` | Logs a warning message. |
| `fn error(msg: str) to Unit` | Logs an error message. |

## OpenClaw Invocation (`OpenClaw.*`)

| Signature | Description |
|-----------|-------------|
| `fn list_skills() to Result[str]` | Lists available OpenClaw skills. |
| `fn call(skill: str, args: str) to Result[str]` | Invokes an OpenClaw skill. |
| `fn subscribe(topic: str) to Result[str]` | Subscribes to an OpenClaw topic. |
| `fn unsubscribe(topic: str) to Result[str]` | Unsubscribes from an OpenClaw topic. |
| `fn notify(topic: str, msg: str) to Result[str]` | Notifies an OpenClaw topic. |

## CDP System Automation (`Browser.*`)

*Note: These are native-script only (not available when compiled to WASM).*

| Signature | Description |
|-----------|-------------|
| `fn open() to Result[Unit]` | Opens the default automation browser. |
| `fn close() to Result[Unit]` | Closes the automation browser. |
| `fn goto(url: str) to Result[Unit]` | Navigates to a specific URL. |
| `fn click(selector: str) to Result[Unit]` | Clicks on the DOM element matched by selector. |
| `fn fill(selector: str, value: str) to Result[Unit]` | Fills a DOM element with a text value. |
| `fn wait_for(selector: str) to Result[Unit]` | Waits for a selector to appear on the page. |
| `fn text(selector: str) to Result[str]` | Returns the inner text of an element. |
| `fn html(selector: str) to Result[str]` | Returns the inner HTML of an element. |
| `fn screenshot(path: str) to Result[Unit]` | Takes a screenshot and saves it to the path. |

## Network (`std.http.*`)

| Signature | Description |
|-----------|-------------|
| `fn get_text(url: str) to Result[str]` | Submits an HTTP GET request to the target URL and returns the response body as text. |
| `fn post_json(url: str, body: str) to Result[str]` | Submits an HTTP POST request to the target URL with the provided JSON body string. |

---

**Related Topics**:
- [Reference: Database Query Surface](ref-db-surface.md)
- [Explanation: The Runtime](../explanation/expl-runtime.md)
