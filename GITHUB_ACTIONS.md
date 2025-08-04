# GitHub Actions CI/CD Setup

This repository includes several GitHub Actions workflows for building and releasing the Transcriber application across Windows and macOS platforms.

## Workflows

### 1. `ci.yml` - Full Release Build (Signed)
- **Triggers**: Push to main/master, PRs, manual dispatch
- **Platforms**: Windows and macOS  
- **Features**:
  - Code signing support (requires secrets)
  - Creates MSI/NSIS installers for Windows
  - Creates DMG packages for macOS
  - Automatic releases on main branch

### 2. `build-unsigned.yml` - Simple Build (Unsigned)
- **Triggers**: Push to main/master/develop, PRs, manual dispatch
- **Platforms**: Windows and macOS
- **Features**:
  - Builds unsigned executables
  - No code signing required
  - Quick builds for testing

### 3. `build-deno.yml` - Deno-Optimized Build
- **Triggers**: Push to main/master/develop, PRs, manual dispatch
- **Platforms**: Windows and macOS
- **Features**:
  - TypeScript checking with Deno
  - Linting and formatting checks
  - Deno task runner integration

## Required Secrets (for signed builds)

To enable code signing and release builds, add these secrets to your GitHub repository:

### For Windows Code Signing:
- `TAURI_SIGNING_PRIVATE_KEY`: Your Windows code signing certificate (base64 encoded)
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: Password for the signing certificate

### For macOS Code Signing:
- `APPLE_CERTIFICATE`: Your Apple Developer certificate (base64 encoded)
- `APPLE_CERTIFICATE_PASSWORD`: Password for the certificate
- `APPLE_SIGNING_IDENTITY`: Your Apple signing identity
- `APPLE_ID`: Your Apple ID email
- `APPLE_PASSWORD`: App-specific password for your Apple ID
- `APPLE_TEAM_ID`: Your Apple Developer Team ID

## Setting Up Secrets

1. Go to your GitHub repository settings
2. Navigate to "Secrets and variables" → "Actions"
3. Click "New repository secret"
4. Add each required secret with its corresponding value

## Deno Integration

The project is configured to work with both npm and Deno:

- `deno.json` contains task definitions and compiler options
- Use `deno task build` instead of `npm run build` when using Deno
- Deno handles TypeScript checking, linting, and formatting

## Usage

### Running Builds Locally

```bash
# Using npm
npm install
npm run build
npm run tauri build

# Using Deno
deno task build
deno task tauri:build
```

### Triggering Builds

- **Automatic**: Push commits to main/master/develop branches
- **Manual**: Use the "Actions" tab in GitHub and click "Run workflow"
- **Pull Requests**: Builds automatically run for all PRs

## Artifacts

Built applications are uploaded as artifacts and can be downloaded from the Actions tab:

- **Windows**: `.msi` and `.exe` installers
- **macOS**: `.dmg` packages and `.app` bundles

## Notes

- The unsigned build workflow is recommended for development and testing
- Signed builds require proper certificates and are recommended for production releases
- Deno workflows include additional TypeScript validation and code quality checks
- All workflows cache dependencies to improve build times
