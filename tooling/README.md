# The tooling crate.

This crate is responsible for all tooling. Note: the assmbler, disassembler and emulator are not a part of this crate.  

## Tooling

- [ ] Linting
- [ ] Formatting
- [ ] Debugging
  - [ ] Breakpoints
  - [ ] Stepping
  - [ ] Variable inspection
  - [ ] Memory inspection
  - [ ] Registers inspection
  - [ ] Call stack inspection
  - [ ] Logging
  - [ ] Profiling
  - [ ] Performance analysis
  - [ ] Code coverage
  - [ ] Heatmaps
  - [ ] Performance metrics
- [ ] Testing
- [ ] Project setup and management

Below only one thing is listed as its the most important and the rest will be added later.

## Project setup and management

This portion will consist of commands such as `pkasm init` to initialize a new project, `pkasm build` to build the project, and `pkasm run` to run the project.

### pkasm init

The `pkasm init` command will initialize a new project.

It will create a pkasm.conf, pkasm.lock, and a src folder with a main.asm file. The pkasm.conf will contain the project name, version, and dependencies. The pkasm.lock will contain the resolved dependencies and their versions.

Example structure of a new project:

```
my_project/
├── pkasm.conf
├── pkasm.lock
├── .gitignore
└── src/
    └── main.asm
    
```

### pkasm build & run

These will not be used till the assembler is complete.