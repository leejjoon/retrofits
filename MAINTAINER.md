# Maintainer Guide

## Building Static Binaries

The project provides a `Dockerfile.static` to build a fully standalone, statically-linked binary for Linux x86_64 using `musl`. This is ideal for distribution as it has zero runtime dependencies (not even `glibc`).

### Prerequisites

- Podman or Docker

### Build Instructions

To build the static image:

```bash
podman build -t retrofits-static -f Dockerfile.static .
```

### Extracting the Binary

Since the final image is built `FROM scratch`, you can extract the binary to your local host using the following steps:

```bash
# 1. Create a temporary container
podman create --name temp-retrofits retrofits-static

# 2. Copy the binary to your host
podman cp temp-retrofits:/retrofits ./retrofits

# 3. Cleanup the temporary container
podman rm temp-retrofits

# 4. Make sure it's executable
chmod +x ./retrofits
```

### Troubleshooting the Build

The static build relies on Alpine Linux to provide static versions of C libraries (`libchafa`, `glib`, `pcre2`, etc.). If dependencies change in `Cargo.toml`, ensure the corresponding `-static` packages are added to the `apk add` command in `Dockerfile.static`.

## Automated Releases

The project uses GitHub Actions to automatically build and release the static Linux binary.

### Triggering a Release

To create a new release:

1.  **Update the version** in `Cargo.toml` if necessary.
2.  **Create and push a tag** starting with `v` (e.g., `v0.1.0`):

    ```bash
    git tag v0.1.0
    git push origin v0.1.0
    ```

The `.github/workflows/release.yml` workflow will:
- Build the static binary using `Dockerfile.static`.
- Extract the binary and calculate its SHA256 checksum.
- Create a new GitHub Release with the binary and checksum as assets.
