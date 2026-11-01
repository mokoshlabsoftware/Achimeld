# Achimeld Wiki authoring workspace

These are the source articles and research records for the website wiki. They are not deployed website pages.

## Read the core guides

1. [Getting started](getting-started.md)
2. [Your first achievement](first-achievement.md)
3. [Achievement General settings](achievements-general.md)
4. [General Images](general-images.md)
5. [Icon editor concepts](icon-editor.md)
6. [Projects and files](projects.md)

The draft articles target application source version 0.2.29.3. The example screenshots and observed controls are from compiled application version 0.2.29.0. [Article metadata](articles.json) records verification scope separately from the documented target; no article has full end-to-end runtime acceptance yet.

## Full reference drafts

- Art foundations: [Layers](icon-layers.md), [Effects](icon-effects.md), [Masks](masks.md), [Gradients and maps](gradients.md).
- Procedural art: [Particles](particles.md), [Explosions](explosions.md), [3D text](text-3d.md), [3D artifacts](artifacts-3d.md).
- Reuse and sources: [Styles](styles.md), [Art Data Bank](art-data-bank.md).
- Game data: [Stats](stats.md), [Localization](localization.md).
- Steam work: [Sync](sync.md), [Steam view](steam.md).
- Outputs and workspace: [Exports](exports.md), [Preferences](preferences.md), [Troubleshooting](troubleshooting.md), [Automation](automation.md).

[Reference coverage](reference-coverage.json) maps all 23 layer kinds and 11 effect kinds to draft destinations. [Phase 3 source evidence](source-evidence-phase3.json) pins the reviewed source files. This maps model kinds and control families; it does not close the individual discovery queue or establish full runtime acceptance.

Run the documentation checks with Node.js:

```text
node build/Check-WikiDocs.mjs .
```

## Research and maintenance

- [Phase 1 inventory and findings](phase-1-inventory.md)
- [Screenshot checklist](screenshot-checklist.md)
- [Source discovery register](source-inventory.json)
- [Capture manifest](screenshots/0.2.29.0/manifest.json)
- [Wiki maintenance and version coverage](../wiki-maintenance.md)

The source register is a lexical discovery queue. Its 8,915 strings include data keys and internal identifiers. They are not 8,915 confirmed controls. Review each candidate before promoting it into a feature reference.

Run the optional documentation utility with Node.js:

```text
node build/Export-WikiInventory.mjs . docs/wiki/source-inventory.json
```

It reads application feature sources, model definitions, top-level developer guides and change history. It writes a dated snapshot with source hashes, model-to-picker comparisons, article suggestions and pending review tasks. It does not run or modify the application. Regeneration replaces the discovery snapshot; preserve reviewed feature decisions in separate authoring records.

Phase 1 has a source inventory and an initial runtime walkthrough. Detailed candidate triage, all conditional states, and historical impact decisions remain open. Phase 2 has six connected draft guides, and phase 3 adds 18 reference drafts. Runtime/control and illustration checks remain open. Website integration belongs to phase 4.


## Phase 5 verification

See [the phase 5 verification pass](phase-5-verification.md) and [machine-readable readiness report](readiness.json). Publication and full runtime acceptance remain pending. Screenshot manifests also include five user-provided images whose application version is unknown; those assets do not inherit the original capture version.
