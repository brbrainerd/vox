---
title: "Distribution Bundles"
description: "What Vox bundles are, how to pick one, and how to roll your own."
category: "Language Reference"
status: "current"
training_eligible: true
---

# Distribution Bundles

A Vox bundle is a tarball with the `vox` host binary plus a curated `plugins/` directory. Every bundle ships *the same* host binary; what differs is which plugins are pre-installed.

## Picking a bundle

| If you want to…                                          | Pick           |
| -------------------------------------------------------- | -------------- |
| Try Vox locally with the default agent skills            | `vox-fullstack`|
| Train ML models locally on NVIDIA hardware               | `vox-ml`       |
| Run a headless backend server with mesh + cloud sync     | `vox-server`   |
| Run a node in a Populi mesh                              | `vox-mesh`     |
| Use Vox as a managed cloud client (no local ML/mesh)     | `vox-cloud-only`|
| Run on edge / on-device with skills only                 | `vox-edge`     |
| Hack on Vox itself with everything available             | `vox-dev`      |
| Build your own custom bundle                             | `vox-base` + plugins |

## Building a bundle

```bash
vox bundle build vox-ml --target linux-x86_64 --out vox-ml-1.0.0-linux-x86_64.tar.gz
```

Bundles are reproducible from the catalog: the same Vox version + same catalog SHA produces byte-identical tarballs.

## Defining your own bundle

External users add a bundle entry in their own catalog overlay (mechanism deferred to a follow-up sub-project) or assemble plugins manually with `vox plugin install <id>` after starting from `vox-base`.

For the first-party bundle list, see the auto-generated [distribution-bundles.generated.md](distribution-bundles.generated.md).
