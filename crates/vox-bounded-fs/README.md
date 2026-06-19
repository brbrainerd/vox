# vox-bounded-fs

Sandboxed, bounded filesystem operations extracted from the [Vox](https://github.com/vox-foundation/vox) project.

Provides UTF-8 file reads capped by a `vox-scaling-policy` size budget, preventing unbounded memory allocation from untrusted or oversized files.

## License

Apache-2.0
