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
