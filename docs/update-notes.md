# Update notes

`CHANGELOG.md` remains the authoritative version history used by Help. Each dated
version contains concise, user-facing improvement bullets. Describe the resulting
benefit; preserve historical entries and avoid inventing changes or categories.

`build/Get-UpdateNotes.ps1` exports this history as JSON, with `schemaVersion`,
`targetVersion`, `sinceVersion`, and a newest-first `releases` array. Every release
has `version`, ISO `date`, and an `improvements` string array.

For an update explanation, pass `-SinceVersion` with the installed working version
and `-TargetVersion` with the package working version. The installed version is
excluded and the target is included. Equal versions produce an empty array.
Without a lower bound, all recorded history through the target is returned.
Unknown versions and reverse ranges fail explicitly rather than producing a
misleading update explanation. An unknown installed version requires the caller
to offer the full recorded history explicitly.

Installers and release tooling can consume this export from their selected source
snapshot and place the resulting JSON beside generated package metadata outside
the repository. The export does not install anything or establish package trust.
Installer display and automatic package attachment are future integration work.

## Startup changes prompt

On launch, Achimeld compares its compiled CHANGELOG working version with the last
acknowledged application version in the per-user APPDATA/Achimeld/rust-about-seen.json
state file. This state is separate from project files, editable preferences and undo.

A newer version opens an in-app prompt after startup settings dialogs. The header
shows the full Achimeld logo, then the compiled version and date on separate lines.
The body lists change bullets since the acknowledged version, including skipped
updates, without individual version headings. Selecting Ok or
pressing Enter/Escape acknowledges the current version using an atomic state write.
An unsuccessful write leaves the version unacknowledged and reports the failure.

The same version does not prompt again. Running an older build retains the newer
acknowledgement, so downgrading does not erase the user's update baseline.
Missing state, legacy boolean state or a baseline absent from the embedded history
shows all recorded change bullets without inventing a range.
Help > Change log remains available for the complete history.

This records the last acknowledged version, not an installer upgrade source.
A version that was never launched or acknowledged does not advance the baseline.
