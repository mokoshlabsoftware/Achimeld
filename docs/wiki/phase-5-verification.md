# Phase 5: verification and publication readiness

Review date: 2026-10-30. Status: In progress. No publication performed.

The website passes nine automated checks covering localization, routes, local links and anchors, search, version labels, canonical URLs, sitemap output and audit failure detection. There are 24 draft articles, 23 layer kinds mapped to references, 11 effect kinds mapped to references and 16 screenshot assets. Five user-provided screenshots have no recorded application version and keep that explicit unknown value.

## Findings and repairs

- Added a repeatable readiness audit in the website checkout: `node scripts/audit-wiki.js "C:\Projects\Achimeld\achimeld"`. Its complete output is [readiness.json](readiness.json).
- The audit checks every screenshot manifest, file signature, article association and version attribution, plus article content hashes and the original source-evidence hashes. Missing manifests for referenced images are errors. A matching file signature is not a visual quality check.
- It uses the application's existing Get-UpdateNotes exporter with an unknown lower bound to inventory the full recorded history. Unassigned entries remain pending, rather than receiving guessed no-impact decisions. An impact-register row is not proof of completed article review.
- Nine evidence files changed since the original review, affecting eight reference articles. Source differences require review; a changed hash alone does not establish which instructions are wrong.
- The particle and explosion articles had newer preset-file notes appended without refreshing their article content identities. Those identities were reconciled after reading the content. Their baseline source-evidence hashes and full-runtime verification remain unchanged.
- The particle guide now links to the illustrated introductory particle-overlay example. Its older single-effect-file explanation is explicitly qualified for versions before 0.2.29.23.
- The application documentation checker now covers all screenshot manifests, including unversioned user-provided images, instead of only the original 11 captures.
- Browser review caught image-driven layout shifts after following a search result into the illustrated tutorial. Screenshot dimensions now reserve space before lazy loading. The particle section settled below the fixed header after navigation. The tutorial was checked at desktop width and at 390 pixels: no horizontal overflow or broken loaded images were detected. Desktop and phone proof images are retained in the website review folder.

## Runtime observation

The running editor's title and interface identify compiled version 0.2.29.23. The application source history was already newer. These identities remain separate from the wiki's original target, 0.2.29.3.

Read-only scene and node inspection confirmed one achievement with a General Image input at ID 0. The visible icon stack contains Explosion, Particles and General Image layers; Particles shows Screen blending. Explosion properties expose Import presets, Save effect and Count range. This is partial observation, not acceptance of file saving, preset application, all layer controls or project persistence.

The example project is dirty and user input was detected during UI inspection. Further app input was stopped to preserve the active editing session. No project save, reopen, remote synchronization or Steam write was performed. Tests requiring an isolated demonstration project remain pending.

## Acceptance queue

| Priority | Work | Acceptance evidence still needed |
| --- | --- | --- |
| 1 | Finish the first-achievement workflow | In a separate test project: create by context menu and image drop, set text and input slot, edit icon, save, reopen and export; verify project companions and actual output names |
| 2 | Review newer controls | Particles, explosions, numeric range switches, masks, shape sizes, color controls and startup update notes; capture the actual compiled version for each observation |
| 3 | Complete historical reconciliation | Record an impact decision for every unassigned change at or below the chosen target; reviewed-through stays unassigned until this is complete |
| 4 | Close illustration and control gaps | 103 outstanding checklist tasks at this pass; the five additional user screenshots support partial tasks without closing unobserved controls |
| 5 | Validate Steam and Sync | A controlled demonstration destination, fresh validation, review/conflict/cancellation/recovery evidence; remote writes require their own intended task |
| 6 | Publish and maintain | Confirm production origin and hosting, deploy an accepted revision, verify public routes/assets/version labels, publish the GitHub README link and record the deployed identity |

The public https://achimeld.com/wiki/ check could not establish current availability: the web tool could not access it and the local HTTP client could not resolve the hostname. The earlier phase 4 result of HTTP 404 is historical evidence only.

The local preview returned HTTP 200 at /wiki/. An old browser error tab was replaced with a fresh verification tab. npm was unavailable in the command environment, so the same syntax checks and all nine tests were run directly with Node and passed.

No article was promoted to Verified or Published, and no documented/verified application version was advanced. This report records a completed verification pass; phase 5 remains in progress until the acceptance and publication work above is complete.
