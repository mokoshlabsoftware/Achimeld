# Troubleshooting and recovery

Start with the visible status/error and the operation that produced it. Preserve local project work before retrying an operation that depends on a saved destination.

## Common problems

| Symptom | Inspect and recover |
| --- | --- |
| Image missing or unavailable | Find it in Art Data Bank, inspect its source and reload the intended file; recheck atlas grids after resizing |
| Icon is unexpectedly blank | Visibility, opacity, masks, source slot, canvas clipping and layer/effect order |
| Icon number differs from the name | Check the Text layer's entered/value-link source separately from the name's {0} template |
| General Image shows the wrong source | Check slot ID, list order and the layer's selected slot |
| No style but icon has artwork | The individual icon still has a recipe; inspect its layers |
| Inspector label is abbreviated | Hover for full text; inspect App style label preferences |
| Layers window is missing | Show layers panel, then Windows > Reset layout if needed |
| Invalid pending fields block actions | Correct them or use Revert to working version; it discards invalid drafts while keeping accepted changes |
| Export refuses a name/folder | Use an empty destination; check invalid/reserved names and collisions |
| Translation import fails | Export fresh IDs/English/Context, keep placeholders, and fix the whole handoff |
| Steam tab is disabled | Check App ID and Sync validation state |
| Counts match but Sync has differences | Counts do not compare field values or icon artwork |
| Previous-session validation is shown | Re-validate; the saved snapshot is not fresh connectivity evidence |
| Interrupted synchronization | Read again and review the reconciliation plan |
| Save was interrupted/files changed externally | Preserve project, companion catalog and artwork; follow recovery guidance rather than overwriting |

## Terminal and logs

Open Tools > Terminal or Preferences > General > View logs. Select the current session or a retained previous-session snapshot. Copy all or select text and Ctrl+C. Ctrl+A selects output.

Pause output/Resume output controls display updates. Clear applies to the current in-app view; use the log folder for retained files. The Terminal is a bounded view, not the full retained history. Its timestamps are UTC.

Preferences > General > Open log folder opens Windows Public Documents/Achimeld/Logs. The current session normally uses log.txt and the previous session log_bak.txt; concurrent/test sessions can use numbered instance files.

A logging failure is shown in the app. Logs retry and bounded in-memory output can remain available; copy relevant output before exit if persistence is failing.

Private crash diagnostics are separate from public text logs. They are local diagnostic artifacts, not a wiki screenshot subject.

## Version and startup notes

At startup, Achimeld checks the latest public GitHub source release in the background. If its four-part version is newer than the running app, a New version available notice appears after other startup notices. Closing it or selecting View on GitHub acknowledges that public version, so it is shown once per new version. A failed or offline check does not interrupt editing; it can be retried on the next launch. The GitHub release contains source only. Ready-made installers are delivered separately.

Help > About and Help > Change log identify the compiled four-part version. Match it to the version label on the wiki article and its screenshots.

From source version 0.2.29.3, a newer launch shows What changed since the last acknowledged version, including skipped versions. Closing or Enter/Escape acknowledges the current version. The same version does not prompt again; a downgrade retains the newer baseline.

Missing/legacy state or a previous version outside recorded history shows the full history with an explanation. A failed acknowledgement write leaves it unacknowledged and reports the failure.

This is remembered application-session state, separate from the project and editable Preferences. The 0.2.29.0 captures do not show the newer startup prompt.

## Reporting an issue

Record the compiled version, affected view, exact action, visible error and whether local edits are saved. Include a focused screenshot and relevant text-log excerpt when useful. Avoid sharing sign-in/session details or unrelated project information.

