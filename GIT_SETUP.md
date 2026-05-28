# Pushing ClipVault to GitHub

Step-by-step instructions for getting the repo live and triggering your first CI run.

---

## 1. Create the GitHub repository

1. Go to https://github.com/new
2. Set **Repository name** to `clipvault`
3. Set visibility to **Public** (required for the free CI minutes tier) or Private
4. **Do not** tick "Add a README" — the repo already has one
5. Click **Create repository** and copy the URL shown (e.g. `https://github.com/YOUR_USERNAME/clipvault.git`)

---

## 2. Update your username in the source files

Username is already set to `Londopy` throughout the repo. The only thing left
is replacing `your@email.com` if you want your actual email in the contact fields
(`LICENSE-COMMERCIAL.adoc`, `SECURITY.md`).

---

## 3. Initialise git and make the first commit

```powershell
cd path\to\clipvault

git init
git add .
git commit -m "Initial commit — ClipVault v0.1.0 scaffold"
```

---

## 4. Connect to GitHub and push

```powershell
git remote add origin https://github.com/Londopy/clipvault.git
git branch -M main
git push -u origin main
```

After this, GitHub Actions will automatically run the CI workflow on every push.

---

## 5. Add GitHub Actions secrets (for the release workflow)

Go to **Settings → Secrets and variables → Actions → New repository secret** and add:

| Secret name            | Value |
|------------------------|-------|
| `GITHUB_TOKEN`         | Auto-provided by GitHub — no action needed |
| `APPLE_CERT`           | Base64-encoded `.p12` certificate (macOS notarization) |
| `APPLE_CERT_PASSWORD`  | Password for the `.p12` |
| `APPLE_ID`             | Your Apple ID email |
| `APPLE_TEAM_ID`        | Your Apple Developer Team ID |

The macOS secrets are only needed if you want to distribute signed `.dmg` files.
Everything else works without them.

---

## 6. Tag and trigger your first release

```powershell
# Bump the version in Cargo.toml first, then:
git add Cargo.toml CHANGELOG.adoc
git commit -m "Release v0.1.0"

git tag -a v0.1.0 -m "ClipVault v0.1.0 — initial release"
git push origin main --tags
```

This triggers `.github/workflows/release.yml`, which:
1. Builds binaries for all 5 targets
2. Generates `checksums.txt`
3. Signs artifacts with cosign
4. Publishes a GitHub Release with auto-generated notes

---

## 7. Recommended branch protection (optional but good practice)

Go to **Settings → Branches → Add rule** for `main`:

- [x] Require a pull request before merging
- [x] Require status checks to pass (select the `CI` workflow)
- [x] Require branches to be up to date before merging
- [x] Do not allow bypassing the above settings

---

## Useful day-to-day commands

```powershell
# Create a feature branch
git checkout -b feature/my-feature

# Push the branch and open a PR
git push -u origin feature/my-feature

# Pull latest main
git checkout main
git pull

# Rebase your branch onto main
git checkout feature/my-feature
git rebase main

# Amend the last commit message
git commit --amend

# Interactive rebase to clean up commits before a PR
git rebase -i HEAD~3
```
