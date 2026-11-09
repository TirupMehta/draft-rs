# Code signing (SignPath Foundation)

Unsigned installers trigger Windows SmartScreen. SignPath Foundation signs
open-source projects for free, with a certificate issued to SignPath Foundation
and a signing policy that only accepts builds from this repository's CI.

## 1. Apply (about 10 minutes, then wait for approval)

Apply at https://signpath.org/apply (Foundation edition). Suggested answers:

* **Project name:** Draft
* **Repository:** https://github.com/TirupMehta/draft-rs
* **License:** MIT (OSI approved)
* **Homepage:** https://draft.tirup.in
* **What it is:** A private, on-device dictation app for Windows 10/11. Hold a
  key, speak, release, and the transcript is pasted at the cursor. Speech
  recognition runs locally with Whisper; audio never leaves the PC.
* **Why signing:** Users download a per-user installer from GitHub Releases.
  Without a signature SmartScreen warns on every download, which discourages
  installation of a privacy tool that has no network activity beyond a one-time,
  checksum-verified model download.
* **Build system:** GitHub Actions (`.github/workflows/release-windows.yml`,
  manual dispatch), public logs, reproducible from the tagged source.
* **Artifacts to sign:** `Draft-Setup-<version>.exe` and
  `Draft-Setup-<version>-offline.exe` (Inno Setup installers).
* **Maintainer:** Tirup Mehta, https://github.com/TirupMehta, https://tirup.in
* **Privacy/telemetry:** none. No analytics, no crash reporting, no update pings.

## 2. After approval

1. In SignPath, create a project from the GitHub Actions trust system and note
   the organization id, project slug, and signing-policy slug. Add an artifact
   configuration that signs `*.exe` in the uploaded zip.
2. In GitHub, repository settings, add:
   * variables `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`,
     `SIGNPATH_POLICY_SLUG`
   * secret `SIGNPATH_API_TOKEN`
3. Run the `release-windows` workflow. When the variables exist it uploads the
   unsigned installers, waits for SignPath, swaps in the signed files, and
   writes the checksums over the signed bytes.
4. Update `draft-web` (`src/lib/site.ts` hashes, the "Not signed yet" labels on
   the download page and support copy) and publish the release with the signed
   installers.

SmartScreen reputation also builds with downloads, so a small warning can
remain for the first days even after signing.
