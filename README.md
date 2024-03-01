# Hyperion

An operating system

## Why the name?

Idk, I asked ChatGPT to give it a name:

> Hyperion is a moon of Saturn that is known for its irregular shape and its rapid rotation.
> It was named after the Titan Hyperion, a figure from Greek mythology who was the father of the sun,
> the moon, and the dawn.
>
> I chose the name Hyperion for your operating system because it sounds modern and powerful, and it
> has a connection to the sun and the moon, which could be seen as representing the dual nature of an
> operating system as both a hardware-level and a software-level entity. Additionally, the irregular
> shape of Hyperion and its rapid rotation could be seen as metaphors for the flexibility and speed
> of a modern operating system.

## How do I run it?

### Dependencies

Packages for Arch:
```bash
pacman -Syu make qemu-system-x86 xorriso jq
```

Rust:
```bash
# rustup:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# bare metal target
rustup target add x86_64-unknown-none
# nightly compiler
rustup toolchain install nightly
```

### In QEMU

It is as simple as just:

```bash
# normal
cargo run
# to see the launch options, run
cargo run -- --help
```

### On HW?

Please don't

## Compiling Rust programs for hyperion (with the std library)

### Building x86_64-unknown-hyperion toolchain
```bash
# clone my rust fork
git clone git@github.com:xor-bits/rust.git
cd rust

# fix the hyperion-syscall path in library/std/Cargo.toml
# if both `rust` and `hyperion` are not in the same parent directory

# configure ./x for building the hyperion cross-compile target
# copy the config.toml base from 'shell.nix' (after `config = pkgs.writeText "rustc-config"`)
# or just use nix-shell

# compile the Rust compiler + std library + some tools for hyperion
# (`src/tools/rustfmt` and `proc-macro-srv-cli` are not needed but they are nice for me)
./x build src/tools/rustfmt proc-macro-srv-cli library compiler

# link the toolchain, so that the installed rustc can use it
rustup toolchain link dev-x86_64-unknown-hyperion build/host/stage1

```

### Compiling with x86_64-unknown-hyperion
```bash
# remove the target dir, if the std library has been recompiled
# (rust doesn't detect that automatically for some reason)
rm -rf ./target/x86_64-unknown-hyperion
# I prefer keeping all build artefacts in one location to speed up compilation and reduce disk use:
#rm -rf $CARGO_HOME/target/x86_64-unknown-hyperion

# compile the package using x86_64-unknown-hyperion
cargo +dev-x86_64-unknown-hyperion build --target=x86_64-unknown-hyperion --bin=std-test
# or if you prefer:
rustup override set dev-x86_64-unknown-hyperion
# and now simply:
cargo build --target=x86_64-unknown-hyperion --package=std-test

# copy the binary to the asset directory (building the kernel will automatically embed it)
cp ./target/x86_64-unknown-hyperion/debug/std-test asset/bin/std-test
#cp $CARGO_HOME/target/x86_64-unknown-hyperion/debug/std-test asset/bin/std-test
```

## Demo(s)

The first kernel side shell:
![image](https://github.com/xor-bits/hyperion/assets/42496863/cde71ecf-825f-4e5b-9a32-f204ffbef6e7)

The second kernel side shell:
![image](https://github.com/xor-bits/hyperion/assets/42496863/76460288-d6d7-47de-ab1b-399d0a91dc80)

The current kernel side shell:
![image](https://github.com/xor-bits/hyperion/assets/42496863/4d59dc17-32fd-478d-91e4-5cb745ff1f2a)

A work in progress window manager + user space shell:
![image](https://github.com/xor-bits/hyperion/assets/42496863/1760a3d0-1c6f-450b-84e6-b7724612facf)

## Related repos:

 - [rust](https://github.com/xor-bits/rust): x86_64-unknown-hyperion Rust toolchain

 - [hyperion-doom](https://github.com/xor-bits/hyperion-doom): Doom ported to hyperion

## Font

The font used contains the first 256 bitmap glyphs from [GNU Unifont](http://unifoundry.com/)
