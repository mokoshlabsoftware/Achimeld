# Change log

The newest entry is the current working version. History starts here; earlier work is not retroactively numbered.

## 0.2.29.0 (2026-10-29)

- Move particle icon decoding out of the runtime UI

## 0.2.28.1 (2026-10-29)

- Prepare bundled particle icons at build time to preserve UI authority boundaries

## 0.2.28.0 (2026-10-29)

- Fix compact effect row regression blocking publication

## 0.2.27.1 (2026-10-29)

- Keep compact effect selection regression compatible with icon-only rows

## 0.2.27.0 (2026-10-29)

- Fix worker cleanup handle validation blocking publication

## 0.2.26.36 (2026-10-29)

- Reject invalid worker cleanup Job handles before Windows enumeration

## 0.2.26.35 (2026-10-29)

- Store runtime text logs in Public Documents and rotate them safely while readers are open

## 0.2.26.34 (2026-10-29)

- Finish successful Steam operations while browser cleanup continues safely in the background

## 0.2.26.33 (2026-10-29)

- Recover Steam browser cleanup automatically without requiring an application restart

## 0.2.26.32 (2026-10-29)

- Start Steam uploads automatically, render icons when needed, and derive locked artwork from the current icon

## 0.2.26.31 (2026-10-29)

- Confirm all owned Steam browser processes exit before reopening and report cleanup failures

## 0.2.26.30 (2026-10-29)

- Keep Steam progress movable while editing artwork, preserve local edits, and show elapsed time in the console font

## 0.2.26.29 (2026-10-29)

- Repair stale Steam synchronization history after deletion and report specific safe synchronization failure reasons

## 0.2.26.28 (2026-10-29)

- Use Consolas consistently in integer and decimal fields throughout the application

## 0.2.26.27 (2026-10-29)

- Center Steam operation progress in a prominent window with larger text and an elapsed timer for validation, deletion, reading and synchronization.

## 0.2.26.26 (2026-10-29)

- Keep unrelated icon previews sharp and preserve their render work when editing or rerolling one icon

## 0.2.26.25 (2026-10-29)

- Restore unindented Shape sliders and smooth and compact the main Explosion knobs

## 0.2.26.24 (2026-10-29)

- Swap the Random and Mask controls in icon layer rows

## 0.2.26.23 (2026-10-29)

- Log Steam workflow stages, browser cleanup, verified step timings and deletion totals

## 0.2.26.22 (2026-10-29)

- Replace Explosion sliders with responsive themed knob banks, configurable small fonts and subtle label-divider grips

## 0.2.26.21 (2026-10-29)

- Use normal resting, hover and focus styling for the Steam deletion confirmation button

## 0.2.26.20 (2026-10-29)

- Add separate Mark all and Clear marks controls to Steam achievements and stats

## 0.2.26.19 (2026-10-29)

- Fix Steam achievement and stat deletion routes and finish validation when browser shutdown stalls

## 0.2.26.18 (2026-10-29)

- Recover rejected edits from the upper bar, preserve saving, and reject non-finite numeric control changes

## 0.2.26.17 (2026-10-29)

- Center Explosion Shape X and Shape Y sliders at 1x, with 0x on the left and 10x on the right.

## 0.2.26.16 (2026-10-29)

- Align layer rows, make mask, random and blend buttons transparent at rest, and move the mask icon before random.

## 0.2.26.15 (2026-10-29)

- Show an animated white radar glow on the style banner while edits affect all style instances.

## 0.2.26.14 (2026-10-29)

- Fix compact layer controls, repeated mask actions, passive image selection changes, and particle icon texture limits.

## 0.2.26.13 (2026-10-29)

- Edit gradient stops and 2D points with compact animated color windows, connected pointers, and direct stop gestures.

## 0.2.26.12 (2026-10-29)

- Simplify particle shape controls by removing redundant headings and showing the current shape in the preset selector.

## 0.2.26.11 (2026-10-29)

- Fix Rust source formatting that blocked the publishing audit

## 0.2.26.10 (2026-10-29)

- Show per-achievement random seed toggles on particle and explosion layer rows and use the normal cursor on layer toolbar actions

## 0.2.26.9 (2026-10-29)

- Add nine blend modes including Color, with generated icons and live previews

## 0.2.26.8 (2026-10-29)

- Keep particle controls available for invalid transforms and constrain particle slider edits

## 0.2.26.7 (2026-10-29)

- Add a visible mask inversion toggle beside the RGBA channel buttons.

## 0.2.26.6 (2026-10-29)

- Add visible layer mask controls, RGBA channel selection, automatic General image references, and matching blend and opacity heights.

## 0.2.26.5 (2026-10-29)

- Show pending achievement and stat API renames with revert, and rename verified Steam records in place during sync.

## 0.2.26.4 (2026-10-29)

- Separate mask actions from other layer menu commands.

## 0.2.26.3 (2026-10-29)

- Delete reviewed Steam entries with fresh checks, skip missing entries and return control to the app after completion or failure.

## 0.2.26.2 (2026-10-29)

- Require current-session Steam validation before opening the Steam panel and return to Sync when validation expires.

## 0.2.26.1 (2026-10-29)

- Highlight Delete on Steam with the marked entry color when entries are marked for deletion.

## 0.2.26.0 (2026-10-28)

- Fix the Steam delete action lint error blocking publication.

## 0.2.25.17 (2026-10-28)

- Fix the Steam delete action lint error blocking publication.

## 0.2.25.16 (2026-10-28)

- Arrange Steam achievements and stats in responsive columns with wider margins and scrollbar clearance.

## 0.2.25.15 (2026-10-28)

- Align Steam and Sync action bars with Art Data Bank and distinguish primary actions with a thin square outline.

## 0.2.25.14 (2026-10-28)

- Add confirmed, undoable forgetting of Steam validation, cached snapshots and unused game banners.

## 0.2.25.13 (2026-10-28)

- Add browser profile folder access and confirmed profile deletion to Sync and Steam.

## 0.2.25.12 (2026-10-28)

- Keep Steam sign-in active while its page redirects after authentication.

## 0.2.25.11 (2026-10-28)

- Allow Steam sign-in requests, open the login form for the selected game, and retain safe failure details.

## 0.2.25.10 (2026-10-28)

- Prevent development packages from reusing stale compiled files after source snapshot changes.

## 0.2.25.9 (2026-10-28)

- Make Steam validation a lightweight sign-in handshake before preparing synchronization artwork.

## 0.2.25.8 (2026-10-28)

- Remove the publisher identity sentence from the installer information page.

## 0.2.25.7 (2026-10-28)

- Restore filled installer maintenance buttons with centered artwork while retaining selection, Next and double-click actions.

## 0.2.25.6 (2026-10-28)

- Close idle development connectors after Build and Run and before installer package replacement

## 0.2.25.5 (2026-10-28)

- Allow bounded Steam audio and video requests and report safe browser request failure details.

## 0.2.25.4 (2026-10-28)

- Assign newly saved icon styles immediately and offer to continue editing the icon or its shared style.

## 0.2.25.3 (2026-10-28)

- Keep the installer unpacking animation moving and allow cancellation during decompression.

## 0.2.25.2 (2026-10-28)

- Select an installer maintenance option with one click and continue with Next or a double click.

## 0.2.25.1 (2026-10-28)

- Close Setup completely after a language handoff so hidden installer processes do not remain.

## 0.2.25.0 (2026-10-28)

- Require re-validation after a previous Steam session, and name the installation an update removes.

## 0.2.24.2 (2026-10-28)

- Show a previous Steam session as its own access state, with a half-grey partner banner and re-validation before synchronization.

## 0.2.24.1 (2026-10-28)

- On update, the installation folder page names the previous installation and says it will be removed while its settings remain.

## 0.2.24.0 (2026-10-27)

- Shrink the filter and hide toolbar labels so a narrower window does not cover the field.

## 0.2.23.1 (2026-10-27)

- Shrink the filter and hide toolbar labels so a narrower window does not cover the field.

## 0.2.23.0 (2026-10-27)

- Review Steam deletion marks and the synchronization plan before changes are applied.

## 0.2.22.12 (2026-10-27)

- Steam achievements and stats can be marked for deletion, and synchronization lists the planned actions before it runs.

## 0.2.22.11 (2026-10-27)

- Add stats in bulk from a JSON list or a plain name list through MCP.

## 0.2.22.10 (2026-10-27)

- State editor uptime, the open project, achievement nodes, Steam-synced nodes, and stats in the MCP handshake.

## 0.2.22.9 (2026-10-27)

- Show the Achimeld mark on the left of the title bar and the name and version on the right, and hide the save status.

## 0.2.22.8 (2026-10-27)

- Advertise the orange Achimeld logo as the MCP server icon.

## 0.2.22.7 (2026-10-27)

- Make the Connect MCP window larger so the setup instructions fit without scrolling.

## 0.2.22.6 (2026-10-27)

- Show the specific installation failure reason on the Setup error page, including when Achimeld is still running.

## 0.2.22.5 (2026-10-27)

- Prevent a second Achimeld installer from running while another setup is active.

## 0.2.22.4 (2026-10-27)

- Bring installer language selection and uninstall progress above other windows when they open.

## 0.2.22.3 (2026-10-27)

- Show the Achimeld logo and installer version in the opening language window.

## 0.2.22.2 (2026-10-27)

- Keep the installer taskbar icon visible by loading its embedded logo and preserving icons when loading fails.

## 0.2.22.1 (2026-10-27)

- Add persistent diagnostics for opening effects and rendering outlines and outer glows.

## 0.2.22.0 (2026-10-27)

- Include the animated startup loading bar in the diagnostic build.

## 0.2.21.0 (2026-10-27)

- Add visible build identity and persistent runtime crash diagnostics.

## 0.2.20.3 (2026-10-27)

- Keep live runtime logs in Windows AppData, retain prior runtime evidence and capture local crash and hang dumps.

## 0.2.20.2 (2026-10-27)

- Show the working version in the top bar and native window title.

## 0.2.20.1 (2026-10-27)

- Show an animated loading bar at the bottom of the startup screen.

## 0.2.20.0 (2026-10-27)

- Make installer completion and recovery consistently use the install folder.

## 0.2.19.6 (2026-10-27)

- Show the final installer destination and centralize verified installer handoff and temporary-copy cleanup.

## 0.2.19.5 (2026-10-27)

- Fix installer Back navigation to reopen language selection and report handoff failures.

## 0.2.19.4 (2026-10-27)

- Fix updates from registered installation folders and make setup failure logs accessible.

## 0.2.19.3 (2026-10-27)

- Show startup loading progress after language selection and restore installer icons and foreground visibility.

## 0.2.19.2 (2026-10-27)

- Center installer maintenance content, enlarge action buttons with icons above labels, and raise the loading display.

## 0.2.19.1 (2026-10-27)

- Center installer loading labels and progress bar in the available space.

## 0.2.19.0 (2026-10-27)

- Fix installer progress rendering and preparation navigation

## 0.2.18.1 (2026-10-27)

- Restore visible orange installer progress, keep navigation out of preparation, and place cancellation in the footer.

## 0.2.18.0 (2026-10-27)

- Update transparent application icons and center installer radio indicators

## 0.2.17.2 (2026-10-27)

- Keep installer radio-button rings and selected centers aligned at every display scale.

## 0.2.17.1 (2026-10-27)

- Use transparent orange application icons and the supplied white glyph throughout the app.

## 0.2.17.0 (2026-10-27)

- Replace opaque installer branding with the transparent orange glyph

## 0.2.16.1 (2026-10-27)

- Use the supplied transparent orange glyph for the installer logo and Windows icon.

## 0.2.16.0 (2026-10-27)

- Finish installer maintenance controls and automatic close after removal

## 0.2.15.1 (2026-10-27)

- Close Setup after uninstall, add Repair/Update/Uninstall icons, join navigation buttons, and use orange progress bars.

## 0.2.15.0 (2026-10-27)

- Use the native Windows installation record for maintenance and removal checks

## 0.2.14.1 (2026-10-27)

- Detect installed folders from the Windows uninstall record and verify complete policy removal.

## 0.2.14.0 (2026-10-26)

- Make installer license selection clear in dark mode.

## 0.2.13.1 (2026-10-26)

- Show a centered solid selection dot in the installer license radio buttons while preserving keyboard selection.

## 0.2.13.0 (2026-10-26)

- Preserve readable installer agreement text in dark mode.

## 0.2.12.1 (2026-10-26)

- Keep the centered installer agreement readable in dark mode by preserving themed text rendering.

## 0.2.12.0 (2026-10-26)

- Fix installer document layout and guard uninstall records, with installed removal acceptance checks.

## 0.2.11.3 (2026-10-26)

- Center installer agreement text and restore visible scrollbars and wrapping on document pages.

## 0.2.11.2 (2026-10-26)

- Reject broken legacy uninstall records and add real installed removal checks for files, shortcuts and network policy.

## 0.2.11.1 (2026-10-26)

- Bring the installer wizard to the foreground when it first opens after language selection.

## 0.2.11.0 (2026-10-26)

- Unify installer output and reuse fresh builds before compilation.

## 0.2.10.15 (2026-10-26)

- Keep one versioned installer in the install folder, reuse unchanged builds, and replace the previous installer only after success.

## 0.2.10.14 (2026-10-26)

- Add named Achimeld uninstall shortcuts, preserve the installation on cancellation or cleanup failure, and validate maintenance uninstall paths.

## 0.2.10.13 (2026-10-26)

- Fix the installer welcome Back button caption in every language and use Powrót in Polish.

## 0.2.10.12 (2026-10-26)

- Remove the redundant existing-folder confirmation when installing, updating or repairing Achimeld.

## 0.2.10.11 (2026-10-26)

- Replace long dashes throughout app and installer translations and explicitly prohibit them in user-facing text.

## 0.2.10.10 (2026-10-26)

- Restore the white transparent Achimeld symbol in the title bar while keeping the orange application icon elsewhere.

## 0.2.10.9 (2026-10-26)

- Add matching folder and document icons to the Open log folder and View logs buttons in Preferences.

## 0.2.10.8 (2026-10-26)

- Replace the installer welcome artwork with the supplied logo-free image, preserving its dimensions.

## 0.2.10.7 (2026-10-26)

- Use the supplied orange Achimeld icon for the application, dialogs, installer, Windows shortcuts and installed-app listing.

## 0.2.10.6 (2026-10-26)

- Show an Already running notice and close duplicate desktop launches while preserving the existing session.

## 0.2.10.5 (2026-10-26)

- Expose live and previous-session logs in Preferences and Terminal, retry logging failures, and record effect rendering and stalled interface work.

## 0.2.10.4 (2026-10-26)

- Fixed Windows installed-app and shortcut icon paths for default and custom installation folders.

## 0.2.10.3 (2026-10-26)

- Reworked Style Viewer with current-project folders, name filtering, editable project layers, protected built-ins, style options, JSON exchange and zoomable previews.

## 0.2.10.2 (2026-10-26)

- Enlarged the installer header logo while preserving its proportions and spacing beside page text.

## 0.2.10.1 (2026-10-26)

- Updated the installer welcome screen with a bold Steam achievements heading and clearer feature description.

## 0.2.10.0 (2026-10-25)

- Separate manual publishing sign-in from browser automation

## 0.2.9.1 (2026-10-25)

- Use a normal Edge window for manual Google sign-in before installer uploads

## 0.2.9.0 (2026-10-25)

- Add versioned Lemon Squeezy installer delivery and standalone upload

## 0.2.8.1 (2026-10-25)

- Automate verified Lemon Squeezy installer delivery with versioned filenames and an upload-only launcher

## 0.2.8.0 (2026-10-25)

- Automate safe GitLab source synchronization during publishing

## 0.2.7.1 (2026-10-25)

- Push committed source to GitLab automatically before publishing

## 0.2.7.0 (2026-10-25)

- Fix publishing checks for gradient collection and inspector width

## 0.2.6.21 (2026-10-25)

- Keep inspector tabs within the selected panel width at fractional display scales

## 0.2.6.20 (2026-10-25)

- Fix the 2D gradient collection code check that blocked publishing

## 0.2.6.19 (2026-10-25)

- Close background editor connectors when the last Achimeld window closes so package files no longer block builds

## 0.2.6.18 (2026-10-25)

- Name the bootstrapper console and explain startup, app monitoring, and shutdown

## 0.2.6.17 (2026-10-25)

- Add saved 2D gradient collection and 1D or 2D particle gradient picker

## 0.2.6.16 (2026-10-25)

- Move 2D gradient point editing into a selection panel with shared color actions

## 0.2.6.15 (2026-10-25)

- Keep color panels above Layers and group color channels at the bottom

## 0.2.6.14 (2026-10-25)

- Add 2D gradient point actions and simpler selection controls

## 0.2.6.13 (2026-10-25)

- Make it easy to keep particles upright with a zero rotation range

## 0.2.6.12 (2026-10-25)

- Describe bootstrapper security in the console and show a smaller logo-only loading screen

## 0.2.6.11 (2026-10-25)

- Show file counts and verification phases while capturing build source

## 0.2.6.10 (2026-10-25)

- Keep development builds stable while source edits continue and detect in-use packages before compiling

## 0.2.6.9 (2026-10-25)

- Identify the Achimeld loading screen as the Achimeld Bootstrapper

## 0.2.6.8 (2026-10-25)

- Display the supplied Achimeld banner at the top of Help About

## 0.2.6.7 (2026-10-25)

- Show the Achimeld loading artwork while the supervised editor starts and hide the launcher console for desktop launches

## 0.2.6.6 (2026-10-25)

- Add a movable window size panel and allow Terminal to extend beyond the app bounds

## 0.2.6.5 (2026-10-25)

- Show the orange Achimeld artwork first and the existing editor image on the installer completion page

## 0.2.6.4 (2026-10-25)

- Display styles as gapless artwork-backed banners with overlaid names and remove the internal Icon Banner caption.

## 0.2.6.3 (2026-10-25)

- Standardize numeric knobs on the compact layer-properties slider and document application-wide usage and opacity exceptions.

## 0.2.6.2 (2026-10-25)

- Reorganize Style Viewer into adjustable banners, a sharp size-controlled preview, and artwork inputs; prevent thumbnail rendering from cancelling style previews.

## 0.2.6.1 (2026-10-25)

- Document Lemon Squeezy test installer delivery and pending store activation

## 0.2.6.0 (2026-10-24)

- Present Achimeld purpose, screenshots and installer options on GitHub

## 0.2.5.1 (2026-10-24)

- Explain Achimeld for game developers with screenshots and installer availability

## 0.2.5.0 (2026-10-23)

- Enforce source-only GitHub releases and keep paid installers local

## 0.2.4.1 (2026-10-23)

- Keep paid installers local and publish all non-private source without installer downloads

## 0.2.4.0 (2026-10-23)

- Use plain versioned GitHub release labels

## 0.2.3.1 (2026-10-23)

- Use plain versioned release names without preview labels

## 0.2.3.0 (2026-10-23)

- Update text-source layout regression for two buttons

## 0.2.2.2 (2026-10-23)

- Validate both text-source buttons across narrow layouts and display scales

## 0.2.2.1 (2026-10-23)

- Stretch Properties tabs across the sidebar width

## 0.2.2.0 (2026-10-23)

- Fix installer recipe compilation

## 0.2.1.1 (2026-10-23)

- Fix installer compilation of installation-folder text

## 0.2.1.0 (2026-10-23)

- Fix publication authentication and complete approved source exports

## 0.2.0.30 (2026-10-23)

- Include required build helpers and original toolbar artwork in public releases

## 0.2.0.29 (2026-10-23)

- Keep dragged canvas nodes attached to the pointer while zooming

## 0.2.0.28 (2026-10-23)

- Add mutually exclusive entered-text and achievement-link source buttons for 2D and 3D text.

## 0.2.0.27 (2026-10-23)

- Allow empty English names and descriptions while editing and saving projects; bound inline validation text.

## 0.2.0.26 (2026-10-23)

- Recover missing publishing sign-in through browser login

## 0.2.0.25 (2026-10-23)

- Document GitHub sign-in troubleshooting for publishing

## 0.2.0.24 (2026-10-23)

- Check GitHub sign-in before starting publication

## 0.2.0.23 (2026-10-23)

- Point the desktop shortcut directly to the current local test build and remove installation options from generated launchers.

## 0.2.0.22 (2026-10-23)

- Fix installer source authentication and release formatting checks

## 0.2.0.21 (2026-10-23)

- Use one fixed Achimeld installation and remove previous protected builds during installer upgrades.

## 0.2.0.20 (2026-10-23)

- Keep Build and Run in current build output and require separate installation for ready builds.

## 0.2.0.19 (2026-10-23)

- Register installed packages in Windows with a policy-aware uninstaller

## 0.2.0.18 (2026-10-23)

- Show existing protected installations before setup and detect duplicate packages before extraction.

## 0.2.0.17 (2026-10-23)

- Show a separate animated loading bar during final package verification and protected installation.

## 0.2.0.16 (2026-10-23)

- Give the installation folder a dedicated review page and always show the installation summary before installing.

## 0.2.0.15 (2026-10-23)

- Preserve installer header artwork proportions instead of squeezing the wordmark into a square.

## 0.2.0.14 (2026-10-23)

- Show cumulative installer build time in minutes and seconds throughout the build

## 0.2.0.13 (2026-10-23)

- Remember validated Steam App IDs after reopening projects and stop repeated validation cues

## 0.2.0.12 (2026-10-23)

- Prevent overlapping development builds from locking the package signing tool

## 0.2.0.11 (2026-10-23)

- Show detailed source capture and verification progress during installer creation

## 0.2.0.10 (2026-10-23)

- Build installers from isolated source snapshots so ongoing edits cannot invalidate the build

## 0.2.0.9 (2026-10-23)

- Add a quick local installer build that skips test suites and allows uncommitted source

## 0.2.0.8 (2026-10-23)

- Stop installer builds on early validation errors and remove an unnecessary storage-check tool dependency

## 0.2.0.7 (2026-10-23)

- Show installer build stages, file progress, live output and elapsed time

## 0.2.0.6 (2026-10-23)

- Reuse verified current installers when building or publishing

## 0.2.0.5 (2026-10-23)

- Add a local installer build shortcut without GitHub publication

## 0.2.0.4 (2026-10-23)

- Make version improvement notes available as structured data for update explanations

## 0.2.0.3 (2026-10-23)

- Document the GitHub PuTTY key location and SSH publishing option.

## 0.2.0.2 (2026-10-23)

- Keep the current published installer in the build root install folder.

## 0.2.0.1 (2026-10-23)

- Add Help > Connect MCP with installation-specific setup instructions and copyable client configuration

## 0.2.0.0 (2026-10-23)

- Start the 0.2 release series.

## 0.1.4.14 (2026-10-23)

- Distinguish saved Steam snapshots from failed validation attempts and show their saved check time

## 0.1.4.13 (2026-10-23)

- Use the supplied achievement artwork in the installer welcome and completion panels.

## 0.1.4.12 (2026-10-23)

- Keep concurrent application diagnostics in the shared log folder and distinguish blocked browser request types

## 0.1.4.11 (2026-10-23)

- Document the shared installed-app log location and live bug investigation workflow

## 0.1.4.10 (2026-10-23)

- Offer an optional desktop shortcut and explain how to pin Achimeld to the Windows taskbar

## 0.1.4.9 (2026-10-23)

- Offer an optional normal-user Achimeld launch when closing the completed installer

## 0.1.4.8 (2026-10-23)

- Present the installer as Achimeld and explain background installation and bootstrapper safeguards

## 0.1.4.7 (2026-10-23)

- Explain the Steam Partner browser connection and distinguish local login sessions from project data in installer notes

## 0.1.4.6 (2026-10-23)

- Use a dark installer with a product introduction, Apache license acceptance, and Mokosh Labs installation folders

## 0.1.4.5 (2026-10-23)

- License Achimeld under Apache 2.0 and include the license in Help, setup and public source.

## 0.1.4.4 (2026-10-23)

- Document the complete private publishing setup and recovery workflow in a dedicated guide

## 0.1.4.3 (2026-10-23)

- Provide one fixed location for the latest published installer with verified checksums and source identity

## 0.1.4.2 (2026-10-23)

- Record the permanent public GitHub checkout and synchronize it after publishing

## 0.1.4.1 (2026-10-23)

- Automatically resize oversized toolbar PNG icons to 128 pixels before builds with smooth antialiased filtering; retain original artwork.

## 0.1.4.0 (2026-10-23)

- Publish through the authenticated GitHub CLI connection

## 0.1.3.1 (2026-10-23)

- Reuse GitHub CLI sign-in for publishing without SSH passphrase prompts

## 0.1.3.0 (2026-10-23)

- Support the dedicated GitLab SSH connection for publishing

## 0.1.2.1 (2026-10-23)

- Use the separate GitLab SSH key while preserving GitHub publishing authentication

## 0.1.2.0 (2026-10-23)

- Add GitLab-only installer publishing and public release snapshots

## 0.1.1.22 (2026-10-23)

- Fix publishing startup with installed tools and pin the GitHub SSH host identity

## 0.1.1.21 (2026-10-23)

- Add GitLab-only publishing tools for self-extracting development installers and public GitHub release snapshots

## 0.1.1.20 (2026-10-21)

- Add a desktop build-guide shortcut alongside the build-and-run and app-only shortcuts.

## 0.1.1.19 (2026-10-21)

- Keep the build shortcut usable after deleting generated outputs and clarify which build folders are disposable.

## 0.1.1.18 (2026-10-20)

- Preserve application diagnostics between runs with a previous-run backup and panic locations.

## 0.1.1.17 (2026-10-20)

- Add the golden trophy as built-in artwork and the default Style Viewer preview image.

## 0.1.1.16 (2026-10-20)

- Remove the extra validation button and keep Steam operation errors visible in red in the Sync view.

## 0.1.1.15 (2026-10-20)

- Discover current Steam game banners from store metadata instead of relying on an obsolete artwork address.

## 0.1.1.14 (2026-10-20)

- Show retired achievements on black with dimmed text and generated status icons that fade with canvas labels.

## 0.1.1.13 (2026-10-20)

- Show full property labels whenever they fit, use smaller regular text and extend the label divider with a centered three-dot grip.

## 0.1.1.12 (2026-10-20)

- Add scrollable and clearable Terminal output, optional automatic visibility and detailed Steam failure timing.

## 0.1.1.11 (2026-10-20)

- Retain completed Steam validation, stop attention stripes and explain unavailable game banners.

## 0.1.1.10 (2026-10-20)

- Keep inspected browser processes tracked through shutdown without repeating failing image queries.

## 0.1.1.9 (2026-10-20)

- Add a reusable UI recipe covering controls, layout, localization, preferences and responsive previews.

## 0.1.1.8 (2026-10-20)

- Explain Steam navigation interruptions and connection failures during project validation.

## 0.1.1.7 (2026-10-20)

- Keep Terminal closed during Steam validation and open it only from Tools.

## 0.1.1.6 (2026-10-20)

- Identify files changed during a build and explain when to retry source archiving and compilation.

## 0.1.1.5 (2026-10-20)

- Outline the selected artwork or mask thumbnail so the active sublayer is clear.

## 0.1.1.4 (2026-10-20)

- Allow masks on every layer with nested editing, undoable right-click references, and simplified layer properties.

## 0.1.1.3 (2026-10-20)

- Add opt-in achievement style collections, MCP style authoring and native 3D cup previews.

## 0.1.1.2 (2026-10-19)

- Preserve imported locked artwork and verified icon comparisons, and display saved Steam counts correctly.

## 0.1.1.1 (2026-10-19)

- Import existing Steam entries automatically during validation and simplify synchronization review.

## 0.1.1.0 (2026-10-19)

- Syncing with Steam

## 0.1.0.24 (2026-10-19)

- Complete guarded Steam editor creation and record reads with clear failure logs and import-first reviews.

## 0.1.0.23 (2026-10-19)

- Keep value-linked icon text blank until an unlock value is assigned.

## 0.1.0.22 (2026-10-19)

- Accept current artwork metadata during synchronization and verify preserved retired data before saving.

## 0.1.0.21 (2026-10-19)

- Keep retired achievements locally and import Steam renames as new cards with owned artwork.

## 0.1.0.20 (2026-10-19)

- Allow Chromium teardown to finish before checking exited process identity.

## 0.1.0.19 (2026-10-19)

- Keep synchronization running when Chromium child processes exit while retaining live-process identity checks.

## 0.1.0.18 (2026-10-19)

- Allow the pinned Steam App ID achievement-count read so validation can finish loading.

## 0.1.0.17 (2026-10-19)

- Support Steamworks script bundle names and open the selected app directly during validation.

## 0.1.0.16 (2026-10-19)

- Fix Steam browser access to its supervised local broker and identify blocked request and network stages.

## 0.1.0.15 (2026-10-19)

- Fix Steam validation startup by safely replacing restored browser tabs while networking is disabled.

## 0.1.0.14 (2026-10-19)

- Add a selectable application Terminal and show Steam validation stages and failure reasons.

## 0.1.0.13 (2026-10-19)

- Keep Steam validation browser cleanup inside supervised processes without opening a failing terminal window.

## 0.1.0.12 (2026-10-19)

- Show a saved-project error before Steam validation instead of opening the Save dialog.

## 0.1.0.11 (2026-10-19)

- Move Steam setup guidance to Validate after entering an App ID and add a generated validation icon.

## 0.1.0.10 (2026-10-19)

- Guide Steam setup through App ID, validation and synchronization with animated button cues and a saved game banner.

## 0.1.0.9 (2026-10-19)

- Keep icon style nodes and art images local and exclude them from achievement counts, automatic naming and synchronization.

## 0.1.0.8 (2026-10-19)

- Keep complete field labels in fixed forms and document when UI abbreviations are allowed.

## 0.1.0.7 (2026-10-19)

- Show separate empty-canvas guidance for achievements and stats.

## 0.1.0.6 (2026-10-18)

- Validate the Steam synchronization destination before applying changes and edit the Steam App ID from Sync.

## 0.1.0.5 (2026-10-18)

- Add General settings to icon style nodes for the displayed unlock value and linked project style.

## 0.1.0.4 (2026-10-18)

- Make unused Layers window surfaces draggable and add Windows > Reset layout to recover floating windows.

## 0.1.0.3 (2026-10-18)

- Highlight the fx button with a subtle fill when a layer has effects.

## 0.1.0.2 (2026-10-18)

- Remove stale generated test outputs and old build copies while retaining current packages and audit evidence.

## 0.1.0.1 (2026-10-18)

- Show the current four-part working version in Help > About.
- Add Help > Change log with dated, embedded change history available offline.
- Add agent rules and an update helper: completed changes increment the fourth number; new commits increment the third number and reset the fourth.

