# The Package Manager

This crate is responsible for managing PkASM packages.

Packages are a way to distribute and share code, libraries, and tools for PkASM.  

## Commands

### pkasm add <pkg>

Adds a new package/dependency to pkasm.conf, resolve, update pkasm.lock

### pkasm remove <pkg>

Removes a package/dependency from pkasm.conf, updates pkasm.lock

### pkasm update [pkg]

Update one or all packages/dependencies in pkasm.conf (within allowed verison ranges), updates pkasm.lock

### pkasm fetch

download everthing in pkasm.lock without building

### pkasm install [pkg]

Installs a package that exposes a CLI/tool globally. pkg can be a package, path. Defaults to local project if no pkg is provided.

### pkasm uninstall <pkg>

Remove a globally installed package/tool

### pkasm search <query>

Searchs the package registry for packages matching the query

### pkasm info [pkg]

show versions, targets, license, deps, etc.

### pkasm tree

Shows a dependency tree of the current project

### pkasm outdated

Lists all packages that have newer versions available

### pkasm publish

Upload current package to the package registry. Requires a valid API key.

### pkasm login/logout

Registry Auth. Uses API keys

### pkasm registry

Alternate registry management

## Package Structure

### pkasm.conf

The main configuration file for a project.

```conf
[package]
name = my_project
version = 0.1.0
license = MIT
author = "Your Name go here" ;; required when publishing to the registry
type = binary ;; can also be library, tool, or plugin

[targets]
arch = x86_64, aarch64, x86, xyz, abc
os = linux, windows, macos, osx

[build]
sources = src/*.asm

[exports]
include = include/

[packages]
crypto = ">=1.0.0, <2.0.0"
cli = ">=0.1.0, <1.0.0"
lib_abc = "0.2.4"
```

### pkasm.lock

The lock file is automatically generated and updated by the package manager. It contains the exact versions of all dependencies used in the project, ensuring that builds are reproducible.

```lock
Nothing here yet
```

### Packages

Packages installed are stored in build/pkgs/<package_name>/  
In this directory you will find the obj files for the package.

Any cache will be put in build/cache/  
Builds will be put in build/bin/<arch>/<os>/binary_name
and build/bin/<arch>/<os>/binary_name.exe (can have file extensions depending on the target OS)

### Object files

We use .obj for object files instead of the default .o extension. This is out of project preference.

## Registry

The package registry is a central repository where packages can be published and shared with the community. It allows users to search for, install, and manage packages easily.

### Accounts

To publish packages to the registry, users must create an account and obtain an API key. The API key is used for authentication when publishing packages.

### Downloading packages

When you download a package from the registry it is downloaded in a compressed format.
That format being a .pkasm file. Interally it is just a .tar.zst file.  
The pkg manager will automatically decompress the file and place it in the correct location.  

All downloads are verified with a SHA512 checksum to ensure the package integrity and authenticity. The checksum is provided by the registry and is checked against the downloaded file before installation.