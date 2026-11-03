# Development APKs

The **Android dev APKs** Actions workflow builds ARM64 on code pushes to `android`.
Documentation-only pushes are skipped. Older in-progress builds are cancelled
when a newer commit arrives. **Run workflow → all_abis** builds all four ABIs.
Pull requests never receive the signing key or trigger this workflow.

These APKs use the optimized Rust `release` profile (including ThinLTO), public
payload checks and official resource downloads. The package is
`dev.horizon.android.dev`, separate from the normal release, and the visible
version contains the run number and source commit. Test data is separate;
worlds and accounts are not automatically copied from the release app.
The workflow does not publish GitHub Releases or send Discord messages.

## One-time maintainer setup

### From a phone (no computer)

The **Android dev signing key (one time)** workflow makes the key on GitHub's runner and stores
it as the secret itself; the key is never shown or downloadable.

1. GitHub > Settings > Developer settings > Fine-grained tokens > Generate: repository access
   **only this fork**, permission **Secrets: Read and write**. Copy the token.
2. The fork > Settings > Secrets and variables > Actions > New repository secret: name
   `DEV_KEY_SETUP_TOKEN`, the token as its value.
3. Actions > **Android dev signing key (one time)** > Run workflow (branch `android`).
4. Delete the `DEV_KEY_SETUP_TOKEN` secret and the token. Then Actions > **Android dev APKs** >
   Run workflow builds the APK.

The key has no backup this way: if the secret is ever replaced, installed dev builds need a
reinstall. With a computer, the commands below keep a copy.

### With a computer


Create a **new, dedicated dev key**, not the release signing key. In PowerShell:

```powershell
keytool -genkeypair -keystore "$env:TEMP/horizon-ci-dev.keystore" -storepass android -keypass android -alias androiddebugkey -keyalg RSA -validity 10000 -dname "CN=Horizon Android Dev CI"
[Convert]::ToBase64String([IO.File]::ReadAllBytes("$env:TEMP/horizon-ci-dev.keystore")) | Set-Clipboard
```

In repository Settings → Secrets and variables → Actions, add
`ANDROID_DEV_KEYSTORE_BASE64` with the clipboard contents. Keep a private backup
of this key outside the repository. The password/alias above match the existing
build configuration. Clear the clipboard afterwards. Never attach the key to an
issue, artifact, release or message.

On Linux or macOS: `keytool -genkeypair -keystore horizon-ci-dev.keystore -storepass android
-keypass android -alias androiddebugkey -keyalg RSA -validity 10000 -dname "CN=Horizon Android
Dev CI"`, then `base64 -w0 horizon-ci-dev.keystore` (macOS: `base64 -i horizon-ci-dev.keystore`)
gives the secret's value.

The workflow fails early if the secret is absent. Keeping the same key permits
in-place updates between CI dev builds; replacing it requires reinstalling the
dev app. Existing locally signed debug APKs may use a different key as well.
Back up their test data before uninstalling. Normal release installations are
unaffected. Do not reset this workflow's run counter: its increasing value is
used as Android's dev-channel version code.

## Sharing with testers

Open Actions → Android dev APKs → the successful run → Artifacts. Share the run
URL in Discord and specify its commit and architecture. Download and unpack the
artifact ZIP, then install its APK. GitHub requires sign-in to download workflow
artifacts; artifacts expire after 14 days. Each contains the APK, checksums,
license notices and `BUILD.json` identifying the exact source and build profile.
Experimental builds have not automatically passed gameplay or performance tests.

Private signing keys, raw Minecraft inputs, prepared resource directories,
player data and local diagnostics are not uploaded as downloadable artifacts.
Only Cargo/Gradle build caches are retained separately for subsequent CI builds.
Cold builds also prepare the generated engine tables, so they may take
substantially longer than an incremental local build.
